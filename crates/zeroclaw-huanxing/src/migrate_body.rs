#[derive(Debug, Clone)]
struct PrimaryTenant {
    user_id: String,
    tenant_dir: String,
}

#[derive(Debug, Clone)]
struct AgentMetadata {
    template: String,
    hasn_id: Option<String>,
}

#[derive(Debug, Clone)]
enum HuanxingRepairOp {
    RemoveEmptyDir {
        path: PathBuf,
    },
    MoveLegacyAgent {
        agent_id: String,
        from: PathBuf,
        to: PathBuf,
        user_id: String,
        template: String,
        hasn_id: Option<String>,
    },
    UpdateUserTenantDir {
        user_id: String,
        tenant_dir: String,
    },
    UpdateAgentHasnId {
        agent_id: String,
        hasn_id: String,
    },
}

#[derive(Debug, Default)]
struct HuanxingRepairPlan {
    primary_tenant: Option<PrimaryTenant>,
    operations: Vec<HuanxingRepairOp>,
    warnings: Vec<String>,
    backup_paths: Vec<PathBuf>,
}

pub async fn migrate_huanxing_unified_instance(config: &Config, apply: bool) -> Result<()> {
    let config_dir = config
        .config_path
        .parent()
        .unwrap_or(&config.workspace_dir)
        .to_path_buf();
    let db_path = config.huanxing.resolve_db_path(&config_dir);
    let plan = plan_huanxing_repair(&config_dir, &db_path, &config.huanxing)?;

    print_huanxing_repair_plan(&config_dir, &db_path, &plan, apply);

    if !apply {
        println!();
        println!("Run `zeroclaw migrate huanxing --apply` to execute the plan above.");
        return Ok(());
    }

    if plan.operations.is_empty() {
        println!();
        println!("Nothing to migrate.");
        return Ok(());
    }

    let backup_dir = create_huanxing_backup(&config_dir, &plan.backup_paths)?;
    println!();
    println!("🛟 Backup created: {}", backup_dir.display());

    apply_huanxing_repair(&db_path, plan)?;
    println!("✅ Unified-instance repair applied");
    Ok(())
}

fn plan_huanxing_repair(
    config_dir: &Path,
    db_path: &Path,
    hx_config: &crate::config::HuanXingConfig,
) -> Result<HuanxingRepairPlan> {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .with_context(|| format!("Failed to open HuanXing db {}", db_path.display()))?;

    let mut plan = HuanxingRepairPlan {
        primary_tenant: load_primary_tenant(&conn)?,
        ..HuanxingRepairPlan::default()
    };

    let legacy_root_default = config_dir.join("agents").join("default");
    if legacy_root_default.exists() {
        if dir_is_empty(&legacy_root_default)? {
            plan.backup_paths.push(legacy_root_default.clone());
            plan.operations.push(HuanxingRepairOp::RemoveEmptyDir {
                path: legacy_root_default,
            });
        } else {
            plan.warnings.push(format!(
                "Legacy root agent dir is not empty, skipped automatic cleanup: {}",
                legacy_root_default.display()
            ));
        }
    }

    let users_default_agents = config_dir.join("users").join("default").join("agents");
    if users_default_agents.exists() {
        match plan.primary_tenant.clone() {
            Some(primary_tenant) => {
                for entry in fs::read_dir(&users_default_agents)? {
                    let entry = entry?;
                    let source_dir = entry.path();
                    if !source_dir.is_dir() {
                        continue;
                    }
                    let agent_id = entry.file_name().to_string_lossy().to_string();
                    let target_dir = hx_config.resolve_agent_wrapper_dir(
                        config_dir,
                        Some(&primary_tenant.tenant_dir),
                        &agent_id,
                    );

                    if target_dir.exists() {
                        plan.warnings.push(format!(
                            "Target agent dir already exists, skipped legacy move: {} -> {}",
                            source_dir.display(),
                            target_dir.display()
                        ));
                        continue;
                    }

                    let metadata = read_agent_metadata(&source_dir).unwrap_or_else(|err| {
                        plan.warnings.push(format!(
                            "Failed to parse agent metadata for {}: {}. Falling back to template=assistant",
                            source_dir.display(),
                            err
                        ));
                        AgentMetadata {
                            template: "assistant".to_string(),
                            hasn_id: None,
                        }
                    });

                    plan.backup_paths.push(source_dir.clone());
                    plan.operations.push(HuanxingRepairOp::MoveLegacyAgent {
                        agent_id: agent_id.clone(),
                        from: source_dir,
                        to: target_dir,
                        user_id: primary_tenant.user_id.clone(),
                        template: metadata.template.clone(),
                        hasn_id: metadata.hasn_id.clone(),
                    });
                }
            }
            None => {
                plan.warnings.push(format!(
                    "Found legacy users/default agents at {}, but no unique formal tenant could be resolved; skipped automatic move",
                    users_default_agents.display()
                ));
            }
        }
    }

    plan.operations
        .extend(plan_db_repairs(&conn, config_dir, hx_config)?);
    if !plan.operations.is_empty() {
        plan.backup_paths.push(db_path.to_path_buf());
    }

    dedup_backup_paths(&mut plan.backup_paths);
    Ok(plan)
}

fn load_primary_tenant(conn: &Connection) -> Result<Option<PrimaryTenant>> {
    let mut stmt = conn.prepare(
        "SELECT user_id, tenant_dir
         FROM users
         WHERE tenant_dir IS NOT NULL
           AND TRIM(tenant_dir) != ''
           AND tenant_dir != 'default'
         ORDER BY datetime(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PrimaryTenant {
                user_id: row.get(0)?,
                tenant_dir: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    match rows.len() {
        0 => Ok(None),
        1 => Ok(rows.into_iter().next()),
        _ => Ok(None),
    }
}

fn plan_db_repairs(
    conn: &Connection,
    config_dir: &Path,
    hx_config: &crate::config::HuanXingConfig,
) -> Result<Vec<HuanxingRepairOp>> {
    let mut ops = Vec::new();

    let mut user_stmt = conn.prepare(
        "SELECT user_id, phone, tenant_dir
         FROM users
         ORDER BY datetime(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, id ASC",
    )?;
    let users = user_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (user_id, phone, tenant_dir) in users {
        let current = tenant_dir.as_deref().map(str::trim).unwrap_or("");
        if !current.is_empty() && current != "default" {
            continue;
        }

        let Some(phone) = phone.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };

        let matches = find_tenant_dirs_for_phone(config_dir, phone)?;
        if matches.len() == 1 {
            ops.push(HuanxingRepairOp::UpdateUserTenantDir {
                user_id,
                tenant_dir: matches[0].clone(),
            });
        }
    }

    let mut agent_stmt = conn.prepare(
        "SELECT a.agent_id, COALESCE(a.hasn_id, ''), u.tenant_dir
         FROM agents a
         JOIN users u ON a.user_id = u.user_id
         ORDER BY datetime(COALESCE(a.created_at, '1970-01-01T00:00:00Z')) ASC, a.id ASC",
    )?;
    let agents = agent_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (agent_id, current_hasn_id, tenant_dir) in agents {
        if !current_hasn_id.trim().is_empty() {
            continue;
        }
        let Some(tenant_dir) = tenant_dir
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };

        let workspace_dir =
            hx_config.resolve_agent_workspace(config_dir, Some(tenant_dir), &agent_id);
        let metadata = match read_agent_metadata(&workspace_dir.parent().unwrap_or(&workspace_dir))
        {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if let Some(hasn_id) = metadata.hasn_id.filter(|value| !value.trim().is_empty()) {
            ops.push(HuanxingRepairOp::UpdateAgentHasnId { agent_id, hasn_id });
        }
    }

    Ok(ops)
}

fn find_tenant_dirs_for_phone(config_dir: &Path, phone: &str) -> Result<Vec<String>> {
    let users_dir = config_dir.join("users");
    if !users_dir.exists() {
        return Ok(Vec::new());
    }

    let suffix = format!("-{phone}");
    let mut matches = Vec::new();
    for entry in fs::read_dir(users_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name != "default" && name.ends_with(&suffix) {
            matches.push(name);
        }
    }
    matches.sort();
    Ok(matches)
}

fn read_agent_metadata(agent_wrapper_dir: &Path) -> Result<AgentMetadata> {
    let canonical_path = agent_wrapper_dir.join("config.toml");
    let legacy_path = agent_wrapper_dir.join("workspace").join("config.toml");
    let config_path = if canonical_path.exists() {
        canonical_path
    } else {
        legacy_path
    };
    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    match content.parse::<toml::Value>() {
        Ok(value) => {
            let agent = value
                .get("agent")
                .and_then(toml::Value::as_table)
                .context("Missing [agent] table in agent config.toml")?;

            let template = agent
                .get("template")
                .and_then(toml::Value::as_str)
                .unwrap_or("assistant")
                .trim()
                .to_string();
            let hasn_id = agent
                .get("hasn_id")
                .and_then(toml::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);

            Ok(AgentMetadata { template, hasn_id })
        }
        Err(_) => parse_agent_metadata_fallback(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display())),
    }
}

fn parse_agent_metadata_fallback(content: &str) -> Result<AgentMetadata> {
    let mut in_agent_table = false;
    let mut template: Option<String> = None;
    let mut hasn_id: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_agent_table = line == "[agent]";
            continue;
        }

        if !in_agent_table {
            continue;
        }

        if let Some(value) = parse_simple_toml_string_value(line, "template") {
            template = Some(value);
            continue;
        }
        if let Some(value) = parse_simple_toml_string_value(line, "hasn_id") {
            let value = value.trim().to_string();
            if !value.is_empty() {
                hasn_id = Some(value);
            }
        }
    }

    Ok(AgentMetadata {
        template: template.unwrap_or_else(|| "assistant".to_string()),
        hasn_id,
    })
}

fn parse_simple_toml_string_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} =");
    let rest = line.strip_prefix(&prefix)?.trim();
    let rest = rest.split('#').next()?.trim();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn print_huanxing_repair_plan(
    config_dir: &Path,
    db_path: &Path,
    plan: &HuanxingRepairPlan,
    apply: bool,
) {
    println!("🧹 ZeroClaw Unified-Instance Repair");
    println!("  Config dir: {}", config_dir.display());
    println!("  DB: {}", db_path.display());
    println!("  Mode: {}", if apply { "apply" } else { "dry-run" });
    match &plan.primary_tenant {
        Some(primary) => println!(
            "  Primary tenant: {} ({})",
            primary.tenant_dir, primary.user_id
        ),
        None => println!("  Primary tenant: unresolved"),
    }

    println!();
    if plan.operations.is_empty() {
        println!("  No automatic changes planned.");
    } else {
        println!("  Planned operations:");
        for op in &plan.operations {
            match op {
                HuanxingRepairOp::RemoveEmptyDir { path } => {
                    println!("    - remove empty legacy dir: {}", path.display());
                }
                HuanxingRepairOp::MoveLegacyAgent {
                    agent_id, from, to, ..
                } => {
                    println!(
                        "    - move legacy agent `{}`: {} -> {}",
                        agent_id,
                        from.display(),
                        to.display()
                    );
                }
                HuanxingRepairOp::UpdateUserTenantDir {
                    user_id,
                    tenant_dir,
                } => {
                    println!(
                        "    - update users.tenant_dir: {} -> {}",
                        user_id, tenant_dir
                    );
                }
                HuanxingRepairOp::UpdateAgentHasnId { agent_id, hasn_id } => {
                    println!("    - sync agents.hasn_id: {} -> {}", agent_id, hasn_id);
                }
            }
        }
    }

    if !plan.warnings.is_empty() {
        println!();
        println!("  Warnings:");
        for warning in &plan.warnings {
            println!("    - {}", warning);
        }
    }
}

fn create_huanxing_backup(config_dir: &Path, backup_paths: &[PathBuf]) -> Result<PathBuf> {
    let backup_root = config_dir.join("migration-backups");
    fs::create_dir_all(&backup_root)?;
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let backup_dir = backup_root.join(format!("huanxing-unified-instance-{timestamp}"));
    fs::create_dir_all(&backup_dir)?;

    for path in backup_paths {
        if !path.exists() {
            continue;
        }
        let relative = path.strip_prefix(config_dir).unwrap_or(path);
        let dest = backup_dir.join(relative);
        copy_recursively(path, &dest)?;
    }

    Ok(backup_dir)
}

fn apply_huanxing_repair(db_path: &Path, plan: HuanxingRepairPlan) -> Result<()> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open HuanXing db {}", db_path.display()))?;
    conn.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| -> Result<()> {
        for op in &plan.operations {
            match op {
                HuanxingRepairOp::RemoveEmptyDir { path } => {
                    if path.exists() && dir_is_empty(path)? {
                        fs::remove_dir_all(path)?;
                    }
                }
                HuanxingRepairOp::MoveLegacyAgent {
                    agent_id,
                    from,
                    to,
                    user_id,
                    template,
                    hasn_id,
                } => {
                    if !from.exists() {
                        continue;
                    }
                    if let Some(parent) = to.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(from, to)?;
                    conn.execute(
                        "INSERT INTO agents (agent_id, user_id, template, hasn_id)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(agent_id) DO UPDATE SET
                             user_id = excluded.user_id,
                             template = excluded.template,
                             hasn_id = COALESCE(excluded.hasn_id, agents.hasn_id),
                             updated_at = datetime('now')",
                        rusqlite::params![agent_id, user_id, template, hasn_id],
                    )?;
                }
                HuanxingRepairOp::UpdateUserTenantDir {
                    user_id,
                    tenant_dir,
                } => {
                    conn.execute(
                        "UPDATE users
                         SET tenant_dir = ?1, updated_at = datetime('now')
                         WHERE user_id = ?2",
                        rusqlite::params![tenant_dir, user_id],
                    )?;
                }
                HuanxingRepairOp::UpdateAgentHasnId { agent_id, hasn_id } => {
                    conn.execute(
                        "UPDATE agents
                         SET hasn_id = ?1, updated_at = datetime('now')
                         WHERE agent_id = ?2",
                        rusqlite::params![hasn_id, agent_id],
                    )?;
                }
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn dedup_backup_paths(paths: &mut Vec<PathBuf>) {
    paths.sort();
    paths.dedup();
}

fn dir_is_empty(path: &Path) -> Result<bool> {
    Ok(fs::read_dir(path)?.next().is_none())
}

fn copy_recursively(source: &Path, dest: &Path) -> Result<()> {
    if source.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, dest)?;
        return Ok(());
    }

    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        copy_recursively(&source_path, &dest_path)?;
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::schema::{Config, MemoryConfig};
    use zeroclaw_memory::SqliteMemory;
    use rusqlite::params;
    use tempfile::TempDir;


        fn seed_huanxing_db(config_dir: &Path) -> PathBuf {
        let db_path = config_dir.join("data").join("users.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id       TEXT NOT NULL UNIQUE,
                phone         TEXT UNIQUE,
                nickname      TEXT,
                tenant_dir    TEXT,
                status        TEXT DEFAULT 'active',
                plan          TEXT DEFAULT 'star_dust',
                plan_expires  TEXT,
                access_token  TEXT,
                llm_token     TEXT,
                gateway_token TEXT,
                token_expires TEXT,
                server_id     TEXT,
                created_at    DATETIME DEFAULT (datetime('now')),
                updated_at    DATETIME DEFAULT (datetime('now')),
                last_active   TEXT
            );
            CREATE TABLE agents (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id      TEXT NOT NULL UNIQUE,
                user_id       TEXT NOT NULL,
                template      TEXT NOT NULL DEFAULT 'assistant',
                star_name     TEXT,
                hasn_id       TEXT UNIQUE,
                status        TEXT DEFAULT 'active',
                created_at    DATETIME DEFAULT (datetime('now')),
                updated_at    DATETIME DEFAULT (datetime('now'))
            );
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (user_id, phone, tenant_dir, created_at)
             VALUES (?1, ?2, ?3, '2026-04-02T00:00:00Z')",
            params!["18611348367", "18611348367", "001-18611348367"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agents (agent_id, user_id, template, hasn_id, created_at)
             VALUES (?1, ?2, ?3, ?4, '2026-04-02T00:00:00Z')",
            params!["default", "18611348367", "assistant", ""],
        )
        .unwrap();
        db_path
    }

        fn write_agent_config(agent_wrapper_dir: &Path, agent_id: &str, template: &str, hasn_id: &str) {
        let workspace_dir = agent_wrapper_dir.join("workspace");
        fs::create_dir_all(&workspace_dir).unwrap();
        fs::write(
            agent_wrapper_dir.join("config.toml"),
            format!(
                "[agent]\nname = \"{agent_id}\"\ntemplate = \"{template}\"\nhasn_id = \"{hasn_id}\"\n"
            ),
        )
        .unwrap();
    }

        #[test]
    fn read_agent_metadata_prefers_wrapper_config() {
        let dir = TempDir::new().unwrap();
        let wrapper_dir = dir.path().join("assistant-130");
        let workspace_dir = wrapper_dir.join("workspace");
        fs::create_dir_all(&workspace_dir).unwrap();

        fs::write(
            wrapper_dir.join("config.toml"),
            "[agent]\ntemplate = \"wrapper-template\"\nhasn_id = \"wrapper-hasn\"\n",
        )
        .unwrap();
        fs::write(
            workspace_dir.join("config.toml"),
            "[agent]\ntemplate = \"legacy-template\"\nhasn_id = \"legacy-hasn\"\n",
        )
        .unwrap();

        let metadata = read_agent_metadata(&wrapper_dir).unwrap();

        assert_eq!(metadata.template, "wrapper-template");
        assert_eq!(metadata.hasn_id.as_deref(), Some("wrapper-hasn"));
    }

        #[test]
    fn read_agent_metadata_falls_back_to_workspace_config() {
        let dir = TempDir::new().unwrap();
        let wrapper_dir = dir.path().join("assistant-130");
        let workspace_dir = wrapper_dir.join("workspace");
        fs::create_dir_all(&workspace_dir).unwrap();

        fs::write(
            workspace_dir.join("config.toml"),
            "[agent]\ntemplate = \"legacy-template\"\nhasn_id = \"legacy-hasn\"\n",
        )
        .unwrap();

        let metadata = read_agent_metadata(&wrapper_dir).unwrap();

        assert_eq!(metadata.template, "legacy-template");
        assert_eq!(metadata.hasn_id.as_deref(), Some("legacy-hasn"));
    }

        #[test]
    fn plan_huanxing_repair_detects_legacy_agent_move() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path();
        let db_path = seed_huanxing_db(config_dir);

        fs::create_dir_all(config_dir.join("agents").join("default")).unwrap();
        write_agent_config(
            &config_dir
                .join("users")
                .join("default")
                .join("agents")
                .join("assistant-130"),
            "assistant-130",
            "assistant",
            "",
        );

        let plan = plan_huanxing_repair(
            config_dir,
            &db_path,
            &crate::config::HuanXingConfig::default(),
        )
        .unwrap();

        assert!(plan.operations.iter().any(|op| matches!(
            op,
            HuanxingRepairOp::RemoveEmptyDir { path }
                if path == &config_dir.join("agents").join("default")
        )));
        assert!(plan.operations.iter().any(|op| matches!(
            op,
            HuanxingRepairOp::MoveLegacyAgent { agent_id, .. } if agent_id == "assistant-130"
        )));
        assert!(plan.backup_paths.iter().any(|path| path == &db_path));
    }

        #[test]
    fn apply_huanxing_repair_moves_agent_and_upserts_db() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path();
        let db_path = seed_huanxing_db(config_dir);

        fs::create_dir_all(config_dir.join("agents").join("default")).unwrap();
        let legacy_agent_dir = config_dir
            .join("users")
            .join("default")
            .join("agents")
            .join("assistant-130");
        write_agent_config(
            &legacy_agent_dir,
            "assistant-130",
            "assistant",
            "hasn-local-1",
        );

        let plan = plan_huanxing_repair(
            config_dir,
            &db_path,
            &crate::config::HuanXingConfig::default(),
        )
        .unwrap();
        apply_huanxing_repair(&db_path, plan).unwrap();

        assert!(!config_dir.join("agents").join("default").exists());
        assert!(!legacy_agent_dir.exists());

        let target_agent_dir = config_dir
            .join("users")
            .join("001-18611348367")
            .join("agents")
            .join("assistant-130");
        assert!(target_agent_dir.exists());

        let conn = Connection::open(&db_path).unwrap();
        let row: (String, String, Option<String>) = conn
            .query_row(
                "SELECT user_id, template, hasn_id FROM agents WHERE agent_id = ?1",
                params!["assistant-130"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row.0, "18611348367");
        assert_eq!(row.1, "assistant");
        assert_eq!(row.2.as_deref(), Some("hasn-local-1"));
    }
}

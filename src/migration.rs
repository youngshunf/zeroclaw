use crate::config::Config;
use crate::memory::{self, Memory, MemoryCategory};
use anyhow::{Context, Result, bail};
use directories::UserDirs;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct SourceEntry {
    key: String,
    content: String,
    category: MemoryCategory,
}

#[derive(Debug, Default)]
struct MigrationStats {
    from_sqlite: usize,
    from_markdown: usize,
    imported: usize,
    skipped_unchanged: usize,
    renamed_conflicts: usize,
}

pub async fn handle_command(command: crate::MigrateCommands, config: &Config) -> Result<()> {
    match command {
        crate::MigrateCommands::Openclaw { source, dry_run } => {
            migrate_openclaw_memory(config, source, dry_run).await
        }
        #[cfg(feature = "huanxing")]
        crate::MigrateCommands::Huanxing { apply } => {
            migrate_huanxing_unified_instance(config, apply).await
        }
    }
}

#[cfg(feature = "huanxing")]
#[derive(Debug, Clone)]
struct PrimaryTenant {
    user_id: String,
    tenant_dir: String,
}

#[cfg(feature = "huanxing")]
#[derive(Debug, Clone)]
struct AgentMetadata {
    template: String,
    hasn_id: Option<String>,
}

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
#[derive(Debug, Default)]
struct HuanxingRepairPlan {
    primary_tenant: Option<PrimaryTenant>,
    operations: Vec<HuanxingRepairOp>,
    warnings: Vec<String>,
    backup_paths: Vec<PathBuf>,
}

#[cfg(feature = "huanxing")]
async fn migrate_huanxing_unified_instance(config: &Config, apply: bool) -> Result<()> {
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

#[cfg(feature = "huanxing")]
fn plan_huanxing_repair(
    config_dir: &Path,
    db_path: &Path,
    hx_config: &crate::huanxing::config::HuanXingConfig,
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
fn plan_db_repairs(
    conn: &Connection,
    config_dir: &Path,
    hx_config: &crate::huanxing::config::HuanXingConfig,
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
fn parse_simple_toml_string_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} =");
    let rest = line.strip_prefix(&prefix)?.trim();
    let rest = rest.split('#').next()?.trim();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
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

#[cfg(feature = "huanxing")]
fn dedup_backup_paths(paths: &mut Vec<PathBuf>) {
    paths.sort();
    paths.dedup();
}

#[cfg(feature = "huanxing")]
fn dir_is_empty(path: &Path) -> Result<bool> {
    Ok(fs::read_dir(path)?.next().is_none())
}

#[cfg(feature = "huanxing")]
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

async fn migrate_openclaw_memory(
    config: &Config,
    source_workspace: Option<PathBuf>,
    dry_run: bool,
) -> Result<()> {
    let source_workspace = resolve_openclaw_workspace(source_workspace)?;
    if !source_workspace.exists() {
        bail!(
            "OpenClaw workspace not found at {}. Pass --source <path> if needed.",
            source_workspace.display()
        );
    }

    if paths_equal(&source_workspace, &config.workspace_dir) {
        bail!("Source workspace matches current ZeroClaw workspace; refusing self-migration");
    }

    let mut stats = MigrationStats::default();
    let entries = collect_source_entries(&source_workspace, &mut stats)?;

    if entries.is_empty() {
        println!(
            "No importable memory found in {}",
            source_workspace.display()
        );
        println!("Checked for: memory/brain.db, MEMORY.md, memory/*.md");
        return Ok(());
    }

    if dry_run {
        println!("🔎 Dry run: OpenClaw migration preview");
        println!("  Source: {}", source_workspace.display());
        println!("  Target: {}", config.workspace_dir.display());
        println!("  Candidates: {}", entries.len());
        println!("    - from sqlite:   {}", stats.from_sqlite);
        println!("    - from markdown: {}", stats.from_markdown);
        println!();
        println!("Run without --dry-run to import these entries.");
        return Ok(());
    }

    if let Some(backup_dir) = backup_target_memory(&config.workspace_dir)? {
        println!("🛟 Backup created: {}", backup_dir.display());
    }

    let memory = target_memory_backend(config)?;

    for (idx, entry) in entries.into_iter().enumerate() {
        let mut key = entry.key.trim().to_string();
        if key.is_empty() {
            key = format!("openclaw_{idx}");
        }

        if let Some(existing) = memory.get(&key).await? {
            if existing.content.trim() == entry.content.trim() {
                stats.skipped_unchanged += 1;
                continue;
            }

            let renamed = next_available_key(memory.as_ref(), &key).await?;
            key = renamed;
            stats.renamed_conflicts += 1;
        }

        memory
            .store(&key, &entry.content, entry.category, None)
            .await?;
        stats.imported += 1;
    }

    println!("✅ OpenClaw memory migration complete");
    println!("  Source: {}", source_workspace.display());
    println!("  Target: {}", config.workspace_dir.display());
    println!("  Imported:         {}", stats.imported);
    println!("  Skipped unchanged:{}", stats.skipped_unchanged);
    println!("  Renamed conflicts:{}", stats.renamed_conflicts);
    println!("  Source sqlite rows:{}", stats.from_sqlite);
    println!("  Source markdown:   {}", stats.from_markdown);

    Ok(())
}

fn target_memory_backend(config: &Config) -> Result<Box<dyn Memory>> {
    memory::create_memory_for_migration(&config.memory.backend, &config.workspace_dir)
}

fn collect_source_entries(
    source_workspace: &Path,
    stats: &mut MigrationStats,
) -> Result<Vec<SourceEntry>> {
    let mut entries = Vec::new();

    let sqlite_path = source_workspace.join("memory").join("brain.db");
    let sqlite_entries = read_openclaw_sqlite_entries(&sqlite_path)?;
    stats.from_sqlite = sqlite_entries.len();
    entries.extend(sqlite_entries);

    let markdown_entries = read_openclaw_markdown_entries(source_workspace)?;
    stats.from_markdown = markdown_entries.len();
    entries.extend(markdown_entries);

    // De-dup exact duplicates to make re-runs deterministic.
    let mut seen = HashSet::new();
    entries.retain(|entry| {
        let sig = format!("{}\u{0}{}\u{0}{}", entry.key, entry.content, entry.category);
        seen.insert(sig)
    });

    Ok(entries)
}

fn read_openclaw_sqlite_entries(db_path: &Path) -> Result<Vec<SourceEntry>> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("Failed to open source db {}", db_path.display()))?;

    let table_exists: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='memories' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if table_exists.is_none() {
        return Ok(Vec::new());
    }

    let columns = table_columns(&conn, "memories")?;
    let key_expr = pick_column_expr(&columns, &["key", "id", "name"], "CAST(rowid AS TEXT)");
    let Some(content_expr) =
        pick_optional_column_expr(&columns, &["content", "value", "text", "memory"])
    else {
        bail!("OpenClaw memories table found but no content-like column was detected");
    };
    let category_expr = pick_column_expr(&columns, &["category", "kind", "type"], "'core'");

    let sql = format!(
        "SELECT {key_expr} AS key, {content_expr} AS content, {category_expr} AS category FROM memories"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    let mut entries = Vec::new();
    let mut idx = 0_usize;

    while let Some(row) = rows.next()? {
        let key: String = row
            .get(0)
            .unwrap_or_else(|_| format!("openclaw_sqlite_{idx}"));
        let content: String = row.get(1).unwrap_or_default();
        let category_raw: String = row.get(2).unwrap_or_else(|_| "core".to_string());

        if content.trim().is_empty() {
            continue;
        }

        entries.push(SourceEntry {
            key: normalize_key(&key, idx),
            content: content.trim().to_string(),
            category: parse_category(&category_raw),
        });

        idx += 1;
    }

    Ok(entries)
}

fn read_openclaw_markdown_entries(source_workspace: &Path) -> Result<Vec<SourceEntry>> {
    let mut all = Vec::new();

    let core_path = source_workspace.join("MEMORY.md");
    if core_path.exists() {
        let content = fs::read_to_string(&core_path)?;
        all.extend(parse_markdown_file(
            &core_path,
            &content,
            MemoryCategory::Core,
            "openclaw_core",
        ));
    }

    let daily_dir = source_workspace.join("memory");
    if daily_dir.exists() {
        for file in fs::read_dir(&daily_dir)? {
            let file = file?;
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("openclaw_daily");
            all.extend(parse_markdown_file(
                &path,
                &content,
                MemoryCategory::Daily,
                stem,
            ));
        }
    }

    Ok(all)
}

#[allow(clippy::needless_pass_by_value)]
fn parse_markdown_file(
    _path: &Path,
    content: &str,
    default_category: MemoryCategory,
    stem: &str,
) -> Vec<SourceEntry> {
    let mut entries = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let line = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let (key, text) = match parse_structured_memory_line(line) {
            Some((k, v)) => (normalize_key(k, idx), v.trim().to_string()),
            None => (
                format!("openclaw_{stem}_{}", idx + 1),
                line.trim().to_string(),
            ),
        };

        if text.is_empty() {
            continue;
        }

        entries.push(SourceEntry {
            key,
            content: text,
            category: default_category.clone(),
        });
    }

    entries
}

fn parse_structured_memory_line(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with("**") {
        return None;
    }

    let rest = line.strip_prefix("**")?;
    let key_end = rest.find("**:")?;
    let key = rest.get(..key_end)?.trim();
    let value = rest.get(key_end + 3..)?.trim();

    if key.is_empty() || value.is_empty() {
        return None;
    }

    Some((key, value))
}

fn parse_category(raw: &str) -> MemoryCategory {
    match raw.trim().to_ascii_lowercase().as_str() {
        "core" | "" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
        other => MemoryCategory::Custom(other.to_string()),
    }
}

fn normalize_key(key: &str, fallback_idx: usize) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return format!("openclaw_{fallback_idx}");
    }
    trimmed.to_string()
}

async fn next_available_key(memory: &dyn Memory, base: &str) -> Result<String> {
    for i in 1..=10_000 {
        let candidate = format!("{base}__openclaw_{i}");
        if memory.get(&candidate).await?.is_none() {
            return Ok(candidate);
        }
    }

    bail!("Unable to allocate non-conflicting key for '{base}'")
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    let mut cols = Vec::new();
    for col in rows {
        cols.push(col?.to_ascii_lowercase());
    }

    Ok(cols)
}

fn pick_optional_column_expr(columns: &[String], candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|candidate| columns.iter().any(|c| c == *candidate))
        .map(std::string::ToString::to_string)
}

fn pick_column_expr(columns: &[String], candidates: &[&str], fallback: &str) -> String {
    pick_optional_column_expr(columns, candidates).unwrap_or_else(|| fallback.to_string())
}

fn resolve_openclaw_workspace(source: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(src) = source {
        return Ok(src);
    }

    let home = UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .context("Could not find home directory")?;

    Ok(home.join(".openclaw").join("workspace"))
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn backup_target_memory(workspace_dir: &Path) -> Result<Option<PathBuf>> {
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_root = workspace_dir
        .join("memory")
        .join("migrations")
        .join(format!("openclaw-{timestamp}"));

    let mut copied_any = false;
    fs::create_dir_all(&backup_root)?;

    let files_to_copy = [
        workspace_dir.join("memory").join("brain.db"),
        workspace_dir.join("MEMORY.md"),
    ];

    for source in files_to_copy {
        if source.exists() {
            let Some(name) = source.file_name() else {
                continue;
            };
            fs::copy(&source, backup_root.join(name))?;
            copied_any = true;
        }
    }

    let daily_dir = workspace_dir.join("memory");
    if daily_dir.exists() {
        let daily_backup = backup_root.join("daily");
        for file in fs::read_dir(&daily_dir)? {
            let file = file?;
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            fs::create_dir_all(&daily_backup)?;
            let Some(name) = path.file_name() else {
                continue;
            };
            fs::copy(&path, daily_backup.join(name))?;
            copied_any = true;
        }
    }

    if copied_any {
        Ok(Some(backup_root))
    } else {
        let _ = fs::remove_dir_all(&backup_root);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, MemoryConfig};
    use crate::memory::SqliteMemory;
    use rusqlite::params;
    use tempfile::TempDir;

    fn test_config(workspace: &Path) -> Config {
        Config {
            workspace_dir: workspace.to_path_buf(),
            config_path: workspace.join("config.toml"),
            memory: MemoryConfig {
                backend: "sqlite".to_string(),
                ..MemoryConfig::default()
            },
            ..Config::default()
        }
    }

    #[test]
    fn parse_structured_markdown_line() {
        let line = "**user_pref**: likes Rust";
        let parsed = parse_structured_memory_line(line).unwrap();
        assert_eq!(parsed.0, "user_pref");
        assert_eq!(parsed.1, "likes Rust");
    }

    #[test]
    fn parse_unstructured_markdown_generates_key() {
        let entries = parse_markdown_file(
            Path::new("/tmp/MEMORY.md"),
            "- plain note",
            MemoryCategory::Core,
            "core",
        );
        assert_eq!(entries.len(), 1);
        assert!(entries[0].key.starts_with("openclaw_core_"));
        assert_eq!(entries[0].content, "plain note");
    }

    #[test]
    fn sqlite_reader_supports_legacy_value_column() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("brain.db");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute_batch("CREATE TABLE memories (key TEXT, value TEXT, type TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO memories (key, value, type) VALUES (?1, ?2, ?3)",
            params!["legacy_key", "legacy_value", "daily"],
        )
        .unwrap();

        let rows = read_openclaw_sqlite_entries(&db_path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "legacy_key");
        assert_eq!(rows[0].content, "legacy_value");
        assert_eq!(rows[0].category, MemoryCategory::Daily);
    }

    #[tokio::test]
    async fn migration_renames_conflicting_key() {
        let source = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Existing target memory
        let target_mem = SqliteMemory::new(target.path()).unwrap();
        target_mem
            .store("k", "new value", MemoryCategory::Core, None)
            .await
            .unwrap();

        // Source sqlite with conflicting key + different content
        let source_db_dir = source.path().join("memory");
        fs::create_dir_all(&source_db_dir).unwrap();
        let source_db = source_db_dir.join("brain.db");
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch("CREATE TABLE memories (key TEXT, content TEXT, category TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO memories (key, content, category) VALUES (?1, ?2, ?3)",
            params!["k", "old value", "core"],
        )
        .unwrap();

        let config = test_config(target.path());
        migrate_openclaw_memory(&config, Some(source.path().to_path_buf()), false)
            .await
            .unwrap();

        let all = target_mem.list(None, None).await.unwrap();
        assert!(all.iter().any(|e| e.key == "k" && e.content == "new value"));
        assert!(
            all.iter()
                .any(|e| e.key.starts_with("k__openclaw_") && e.content == "old value")
        );
    }

    #[tokio::test]
    async fn dry_run_does_not_write() {
        let source = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let source_db_dir = source.path().join("memory");
        fs::create_dir_all(&source_db_dir).unwrap();

        let source_db = source_db_dir.join("brain.db");
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch("CREATE TABLE memories (key TEXT, content TEXT, category TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO memories (key, content, category) VALUES (?1, ?2, ?3)",
            params!["dry", "run", "core"],
        )
        .unwrap();

        let config = test_config(target.path());
        migrate_openclaw_memory(&config, Some(source.path().to_path_buf()), true)
            .await
            .unwrap();

        let target_mem = SqliteMemory::new(target.path()).unwrap();
        assert_eq!(target_mem.count().await.unwrap(), 0);
    }

    #[test]
    fn migration_target_rejects_none_backend() {
        let target = TempDir::new().unwrap();
        let mut config = test_config(target.path());
        config.memory.backend = "none".to_string();

        let err = target_memory_backend(&config)
            .err()
            .expect("backend=none should be rejected for migration target");
        assert!(err.to_string().contains("disables persistence"));
    }

    // ── §7.1 / §7.2 Config backward compatibility & migration tests ──

    #[test]
    fn parse_category_handles_all_variants() {
        assert_eq!(parse_category("core"), MemoryCategory::Core);
        assert_eq!(parse_category("daily"), MemoryCategory::Daily);
        assert_eq!(parse_category("conversation"), MemoryCategory::Conversation);
        assert_eq!(parse_category(""), MemoryCategory::Core);
        assert_eq!(
            parse_category("custom_type"),
            MemoryCategory::Custom("custom_type".to_string())
        );
    }

    #[test]
    fn parse_category_case_insensitive() {
        assert_eq!(parse_category("CORE"), MemoryCategory::Core);
        assert_eq!(parse_category("Daily"), MemoryCategory::Daily);
        assert_eq!(parse_category("CONVERSATION"), MemoryCategory::Conversation);
    }

    #[test]
    fn normalize_key_handles_empty_string() {
        let key = normalize_key("", 42);
        assert_eq!(key, "openclaw_42");
    }

    #[test]
    fn normalize_key_trims_whitespace() {
        let key = normalize_key("  my_key  ", 0);
        assert_eq!(key, "my_key");
    }

    #[test]
    fn parse_structured_markdown_rejects_empty_key() {
        assert!(parse_structured_memory_line("****:value").is_none());
    }

    #[test]
    fn parse_structured_markdown_rejects_empty_value() {
        assert!(parse_structured_memory_line("**key**:").is_none());
    }

    #[test]
    fn parse_structured_markdown_rejects_no_stars() {
        assert!(parse_structured_memory_line("key: value").is_none());
    }

    #[tokio::test]
    async fn migration_skips_empty_content() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("brain.db");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute_batch("CREATE TABLE memories (key TEXT, content TEXT, category TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO memories (key, content, category) VALUES (?1, ?2, ?3)",
            params!["empty_key", "   ", "core"],
        )
        .unwrap();

        let rows = read_openclaw_sqlite_entries(&db_path).unwrap();
        assert_eq!(
            rows.len(),
            0,
            "entries with empty/whitespace content must be skipped"
        );
    }

    #[test]
    fn backup_creates_timestamped_directory() {
        let tmp = TempDir::new().unwrap();
        let mem_dir = tmp.path().join("memory");
        std::fs::create_dir_all(&mem_dir).unwrap();

        // Create a brain.db to back up
        let db_path = mem_dir.join("brain.db");
        std::fs::write(&db_path, "fake db content").unwrap();

        let result = backup_target_memory(tmp.path()).unwrap();
        assert!(
            result.is_some(),
            "backup should be created when files exist"
        );

        let backup_dir = result.unwrap();
        assert!(backup_dir.exists());
        assert!(
            backup_dir.to_string_lossy().contains("openclaw-"),
            "backup dir must contain openclaw- prefix"
        );
    }

    #[test]
    fn backup_returns_none_when_no_files() {
        let tmp = TempDir::new().unwrap();
        let result = backup_target_memory(tmp.path()).unwrap();
        assert!(
            result.is_none(),
            "backup should return None when no files to backup"
        );
    }

    #[cfg(feature = "huanxing")]
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

    #[cfg(feature = "huanxing")]
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

    #[cfg(feature = "huanxing")]
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

    #[cfg(feature = "huanxing")]
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

    #[cfg(feature = "huanxing")]
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
            &crate::huanxing::config::HuanXingConfig::default(),
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

    #[cfg(feature = "huanxing")]
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
            &crate::huanxing::config::HuanXingConfig::default(),
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

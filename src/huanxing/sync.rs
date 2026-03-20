//! Common skills synchronization.
//!
//! On daemon startup, reads `common-skills.yaml` from the hub repository
//! and syncs the listed skills into `common_skills_dir`.
//! Runtime loading does NOT depend on this config — it simply scans the directory.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Parsed common-skills.yaml.
#[derive(Debug, serde::Deserialize)]
struct CommonSkillsConfig {
    #[allow(dead_code)]
    version: Option<String>,
    skills: Vec<String>,
}

/// Read `manifest.yaml` from a skill directory and extract the version string.
async fn read_manifest_version(skill_dir: &Path) -> Option<String> {
    let manifest = skill_dir.join("manifest.yaml");
    let content = tokio::fs::read_to_string(&manifest).await.ok()?;
    // Simple YAML parsing — look for `version: "X.Y.Z"` or `version: X.Y.Z`
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("version:") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Find a skill's source directory in the hub repository.
/// Hub layout: `skills/{category}/{skill_id}/`
async fn find_skill_in_hub(hub_dir: &Path, skill_id: &str) -> Option<PathBuf> {
    let skills_root = hub_dir.join("skills");
    let mut categories = match tokio::fs::read_dir(&skills_root).await {
        Ok(entries) => entries,
        Err(_) => return None,
    };
    while let Ok(Some(cat_entry)) = categories.next_entry().await {
        if !cat_entry
            .file_type()
            .await
            .map(|ft| ft.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let candidate = cat_entry.path().join(skill_id);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// Recursively copy a directory.
async fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dest).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ft = entry.file_type().await?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if ft.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dest_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dest_path).await?;
        }
    }
    Ok(())
}

/// Sync common skills from hub to `common_skills_dir`.
///
/// Returns `(added, updated, removed, skipped)` counts.
pub async fn sync_common_skills(
    hub_dir: &Path,
    common_skills_dir: &Path,
) -> Result<(usize, usize, usize, usize)> {
    let config_path = hub_dir.join("common-skills.yaml");
    if !config_path.exists() {
        tracing::debug!(
            "No common-skills.yaml found at {}, skipping sync",
            config_path.display()
        );
        return Ok((0, 0, 0, 0));
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: CommonSkillsConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    // load_skills() expects skills at `{dir}/skills/{name}/`, so we sync
    // into the `skills/` subdirectory to match the upstream convention.
    let skills_subdir = common_skills_dir.join("skills");
    tokio::fs::create_dir_all(&skills_subdir).await?;

    let desired: HashSet<String> = config.skills.into_iter().collect();
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut removed = 0usize;
    let mut skipped = 0usize;

    // 1. Sync desired skills from hub → common_skills_dir/skills/
    for skill_id in &desired {
        let dest = skills_subdir.join(skill_id);
        let src = match find_skill_in_hub(hub_dir, skill_id).await {
            Some(p) => p,
            None => {
                tracing::warn!(
                    skill = %skill_id,
                    "Common skill not found in hub, skipping"
                );
                continue;
            }
        };

        let hub_version = read_manifest_version(&src).await;
        let local_version = read_manifest_version(&dest).await;

        if dest.exists() && hub_version.is_some() && hub_version == local_version {
            skipped += 1;
            continue;
        }

        // New or version differs → copy
        if dest.exists() {
            // Remove old version
            let _ = tokio::fs::remove_dir_all(&dest).await;
            updated += 1;
            tracing::info!(
                skill = %skill_id,
                from = local_version.as_deref().unwrap_or("?"),
                to = hub_version.as_deref().unwrap_or("?"),
                "Updated common skill"
            );
        } else {
            added += 1;
            tracing::info!(
                skill = %skill_id,
                version = hub_version.as_deref().unwrap_or("?"),
                "Added common skill"
            );
        }

        copy_dir_recursive(&src, &dest)
            .await
            .with_context(|| format!("Failed to copy skill '{}' from hub", skill_id))?;
    }

    // 2. Remove skills in common_skills_dir/skills/ that are NOT in the yaml list
    let mut entries = tokio::fs::read_dir(&skills_subdir).await?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry
            .file_type()
            .await
            .map(|ft| ft.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue; // skip .trash etc
        }
        if !desired.contains(&name) {
            // Move to .trash instead of deleting
            let trash_dir = skills_subdir.join(".trash");
            let _ = tokio::fs::create_dir_all(&trash_dir).await;
            let trash_dest = trash_dir.join(format!(
                "{}-{}",
                name,
                chrono::Utc::now().format("%Y%m%d%H%M%S")
            ));
            match tokio::fs::rename(entry.path(), &trash_dest).await {
                Ok(()) => {
                    removed += 1;
                    tracing::info!(
                        skill = %name,
                        "Removed common skill (moved to .trash)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        skill = %name,
                        error = %e,
                        "Failed to remove obsolete common skill"
                    );
                }
            }
        }
    }

    tracing::info!(
        added,
        updated,
        removed,
        skipped,
        total = desired.len(),
        "Common skills sync complete"
    );

    Ok((added, updated, removed, skipped))
}

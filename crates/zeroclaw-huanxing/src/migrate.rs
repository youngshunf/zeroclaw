//! HuanXing 统一实例迁移工具。
//!
//! 从 huanxing-clean src/migration.rs 的 huanxing-gated 部分搬迁而来。
//! 提供 `zeroclaw migrate huanxing [--apply]` CLI 命令的后端实现：
//! 扫描 `~/.huanxing` 下的旧版平铺目录结构、清理空目录、迁移 agent 数据
//! 到新的多租户路径 `users/{tenant_dir}/agents/{agent_name}/`。

use anyhow::{Context, Result, bail};
use directories::UserDirs;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use zeroclaw_config::schema::Config;
use zeroclaw_memory::{self as memory, Memory, MemoryCategory};

// ── 从 huanxing-clean src/migration.rs 复制过来的辅助类型 ─────
// （SourceEntry / MigrationStats 原本是 openclaw memory 迁移的辅助类型，
//  huanxing_migrate 函数体里 import 通过 super:: 引用，独立到本模块后
//  需要在同一作用域内定义。）

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct SourceEntry {
    key: String,
    content: String,
    category: MemoryCategory,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct MigrationStats {
    from_sqlite: usize,
    from_markdown: usize,
    imported: usize,
    skipped_unchanged: usize,
    renamed_conflicts: usize,
}

include!("migrate_body.rs");

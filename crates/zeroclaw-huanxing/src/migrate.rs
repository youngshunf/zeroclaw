//! HuanXing 统一实例迁移工具。
//!
//! 从 huanxing-clean src/migration.rs 的 huanxing-gated 部分搬迁而来。
//! 提供 `zeroclaw migrate huanxing [--apply]` CLI 命令的后端实现：
//! 扫描 `~/.huanxing` 下的旧版平铺目录结构、清理空目录、迁移 agent 数据
//! 到新的多租户路径 `users/{tenant_dir}/agents/{agent_name}/`。

// openclaw 迁移相关的辅助函数已在 Phase 5.6d 清理（由 include! 从
// huanxing-clean migration.rs 拉进来但 zeroclaw-huanxing 只用 huanxing 部分，
// openclaw 入口在上游 zeroclaw_runtime::migration）。保留的 use 仅覆盖
// huanxing 迁移实际需要的符号。
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use zeroclaw_config::schema::Config;

// 下列类型在去除 openclaw 辅助函数后不再被使用，但 migrate_body.rs 的
// struct 定义仍然引用 MemoryCategory（SourceEntry），所以保留 import。
#[allow(unused_imports)]
use zeroclaw_memory::{Memory, MemoryCategory};

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

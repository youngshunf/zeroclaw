//! HuanXing document management tools (Phase 3).
//!
//! 11 tools for folder + document CRUD + sharing.
//! All calls go through the backend API with Owner Key authentication.
//!
//! Authentication: `Authorization: OwnerKey hasn_ok_xxx`
//! The owner_key is injected at construction time (from user registration).

use crate::huanxing::api_client::ApiClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const DOCS_PREFIX: &str = "/api/v1/huanxing/user/docs";

/// Shared context for all document tools.
#[derive(Clone)]
pub struct DocToolCtx {
    pub api: ApiClient,
    /// HASN Owner API Key (hasn_ok_xxx), injected at construction.
    pub owner_key: String,
}

// Helper: build a ToolResult for API call results.
fn ok_result(data: serde_json::Value) -> ToolResult {
    ToolResult {
        success: true,
        output: data.to_string(),
        error: None,
    }
}

fn err_result(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

// ═══════════════════════════════════════════════════════
// Folder Tools (4)
// ═══════════════════════════════════════════════════════

// ── hx_folder_tree ───────────────────────────────────

pub struct HxFolderTree {
    api: ApiClient,
    owner_key: String,
}

impl HxFolderTree {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxFolderTree {
    fn name(&self) -> &str {
        "hx_folder_tree"
    }
    fn description(&self) -> &str {
        "获取用户的文档目录树。返回完整的树形结构，包含每个目录下的文档数量。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match self
            .api
            .ownerkey_get(&format!("{DOCS_PREFIX}/folders"), &self.owner_key, &[])
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "tree": resp }))),
            Err(e) => Ok(err_result(format!("获取目录树失败: {e}"))),
        }
    }
}

// ── hx_folder_create ─────────────────────────────────

pub struct HxFolderCreate {
    api: ApiClient,
    owner_key: String,
}

impl HxFolderCreate {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxFolderCreate {
    fn name(&self) -> &str {
        "hx_folder_create"
    }
    fn description(&self) -> &str {
        "创建文档目录。可指定父目录以创建子目录，最多支持5层嵌套。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "目录名称" },
                "parent_id": { "type": "number", "description": "父目录ID（不传则创建在根目录）" },
                "icon": { "type": "string", "description": "目录图标（emoji）" },
                "description": { "type": "string", "description": "目录描述" }
            },
            "required": ["name"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let mut body = json!({ "name": args["name"] });
        if let Some(pid) = args["parent_id"].as_i64() {
            body["parent_id"] = json!(pid);
        }
        if let Some(icon) = args["icon"].as_str() {
            body["icon"] = json!(icon);
        }
        if let Some(desc) = args["description"].as_str() {
            body["description"] = json!(desc);
        }
        match self
            .api
            .ownerkey_post(&format!("{DOCS_PREFIX}/folders"), &self.owner_key, &body)
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "folder": resp }))),
            Err(e) => Ok(err_result(format!("创建目录失败: {e}"))),
        }
    }
}

// ── hx_folder_delete ─────────────────────────────────

pub struct HxFolderDelete {
    api: ApiClient,
    owner_key: String,
}

impl HxFolderDelete {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxFolderDelete {
    fn name(&self) -> &str {
        "hx_folder_delete"
    }
    fn description(&self) -> &str {
        "删除文档目录。默认只能删除空目录，设置 recursive=true 可递归删除。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "folder_id": { "type": "number", "description": "目录ID" },
                "recursive": { "type": "boolean", "description": "是否递归删除（默认 false）" }
            },
            "required": ["folder_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let folder_id = args["folder_id"].as_i64().unwrap_or(0);
        let recursive = args["recursive"].as_bool().unwrap_or(false);
        let qs = if recursive { "?recursive=true" } else { "" };
        match self
            .api
            .ownerkey_delete(
                &format!("{DOCS_PREFIX}/folders/{folder_id}{qs}"),
                &self.owner_key,
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "deleted": true, "detail": resp }))),
            Err(e) => Ok(err_result(format!("删除目录失败: {e}"))),
        }
    }
}

// ── hx_folder_move ───────────────────────────────────

pub struct HxFolderMove {
    api: ApiClient,
    owner_key: String,
}

impl HxFolderMove {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxFolderMove {
    fn name(&self) -> &str {
        "hx_folder_move"
    }
    fn description(&self) -> &str {
        "移动目录到另一个父目录下。target_parent_id 不传则移到根目录。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "folder_id": { "type": "number", "description": "要移动的目录ID" },
                "target_parent_id": { "type": "number", "description": "目标父目录ID（不传=根目录）" }
            },
            "required": ["folder_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let folder_id = args["folder_id"].as_i64().unwrap_or(0);
        let body = json!({ "target_parent_id": args["target_parent_id"] });
        match self
            .api
            .ownerkey_post(
                &format!("{DOCS_PREFIX}/folders/{folder_id}/move"),
                &self.owner_key,
                &body,
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "moved": true, "detail": resp }))),
            Err(e) => Ok(err_result(format!("移动目录失败: {e}"))),
        }
    }
}

// ═══════════════════════════════════════════════════════
// Document Tools (7)
// ═══════════════════════════════════════════════════════

// ── hx_doc_list ──────────────────────────────────────

pub struct HxDocList {
    api: ApiClient,
    owner_key: String,
}

impl HxDocList {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocList {
    fn name(&self) -> &str {
        "hx_doc_list"
    }
    fn description(&self) -> &str {
        "获取用户的文档列表。可按目录筛选。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "folder_id": { "type": "number", "description": "目录ID（不传=根目录下的文档）" }
            },
            "required": []
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let mut params: Vec<(&str, &str)> = Vec::new();
        let folder_str;
        if let Some(fid) = args["folder_id"].as_i64() {
            folder_str = fid.to_string();
            params.push(("folder_id", &folder_str));
        }
        match self
            .api
            .ownerkey_get(DOCS_PREFIX, &self.owner_key, &params)
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "documents": resp }))),
            Err(e) => Ok(err_result(format!("获取文档列表失败: {e}"))),
        }
    }
}

// ── hx_doc_get ───────────────────────────────────────

pub struct HxDocGet {
    api: ApiClient,
    owner_key: String,
}

impl HxDocGet {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocGet {
    fn name(&self) -> &str {
        "hx_doc_get"
    }
    fn description(&self) -> &str {
        "获取文档详情，包括标题、内容、标签、字数等信息。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "doc_id": { "type": "number", "description": "文档ID" }
            },
            "required": ["doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        match self
            .api
            .ownerkey_get(
                &format!("{DOCS_PREFIX}/{doc_id}"),
                &self.owner_key,
                &[],
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "document": resp }))),
            Err(e) => Ok(err_result(format!("获取文档失败: {e}"))),
        }
    }
}

// ── hx_doc_create ────────────────────────────────────

pub struct HxDocCreate {
    api: ApiClient,
    owner_key: String,
}

impl HxDocCreate {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocCreate {
    fn name(&self) -> &str {
        "hx_doc_create"
    }
    fn description(&self) -> &str {
        "创建新文档。支持 Markdown 格式内容，可指定目录存放。创建后自动生成24小时分享链接。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "文档标题" },
                "content": { "type": "string", "description": "文档内容（Markdown 格式）" },
                "tags": { "type": "string", "description": "标签（逗号分隔）" },
                "folder_id": { "type": "number", "description": "目录ID" },
                "status": { "type": "string", "description": "状态：draft / published / archived" }
            },
            "required": ["title", "content"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let tags: Vec<&str> = args["tags"]
            .as_str()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let mut body = json!({
            "title": args["title"],
            "content": args["content"].as_str().unwrap_or(""),
            "tags": tags,
            "auto_share": { "permission": "view", "expires_hours": 24 },
        });
        if let Some(fid) = args["folder_id"].as_i64() {
            body["folder_id"] = json!(fid);
        }
        if let Some(status) = args["status"].as_str() {
            body["status"] = json!(status);
        }
        match self
            .api
            .ownerkey_post(DOCS_PREFIX, &self.owner_key, &body)
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "document": resp }))),
            Err(e) => Ok(err_result(format!("创建文档失败: {e}"))),
        }
    }
}

// ── hx_doc_update ────────────────────────────────────

pub struct HxDocUpdate {
    api: ApiClient,
    owner_key: String,
}

impl HxDocUpdate {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocUpdate {
    fn name(&self) -> &str {
        "hx_doc_update"
    }
    fn description(&self) -> &str {
        "更新现有文档。可更新标题、内容、标签、状态等。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "doc_id": { "type": "number", "description": "文档ID" },
                "title": { "type": "string", "description": "新标题" },
                "content": { "type": "string", "description": "新内容（Markdown 格式）" },
                "tags": { "type": "string", "description": "标签（逗号分隔）" },
                "status": { "type": "string", "description": "状态：draft / published / archived" }
            },
            "required": ["doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        let mut body = json!({});
        if let Some(title) = args["title"].as_str() {
            body["title"] = json!(title);
        }
        if let Some(content) = args["content"].as_str() {
            body["content"] = json!(content);
        }
        if let Some(status) = args["status"].as_str() {
            body["status"] = json!(status);
        }
        if let Some(tags_str) = args["tags"].as_str() {
            let tags: Vec<&str> = tags_str
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect();
            body["tags"] = json!(tags);
        }
        match self
            .api
            .ownerkey_put(
                &format!("{DOCS_PREFIX}/{doc_id}"),
                &self.owner_key,
                &body,
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "updated": true, "detail": resp }))),
            Err(e) => Ok(err_result(format!("更新文档失败: {e}"))),
        }
    }
}

// ── hx_doc_delete ────────────────────────────────────

pub struct HxDocDelete {
    api: ApiClient,
    owner_key: String,
}

impl HxDocDelete {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocDelete {
    fn name(&self) -> &str {
        "hx_doc_delete"
    }
    fn description(&self) -> &str {
        "删除指定文档。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "doc_id": { "type": "number", "description": "文档ID" }
            },
            "required": ["doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        match self
            .api
            .ownerkey_delete(
                &format!("{DOCS_PREFIX}/{doc_id}"),
                &self.owner_key,
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "deleted": true, "detail": resp }))),
            Err(e) => Ok(err_result(format!("删除文档失败: {e}"))),
        }
    }
}

// ── hx_doc_move ──────────────────────────────────────

pub struct HxDocMove {
    api: ApiClient,
    owner_key: String,
}

impl HxDocMove {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocMove {
    fn name(&self) -> &str {
        "hx_doc_move"
    }
    fn description(&self) -> &str {
        "移动文档到指定目录。target_folder_id 不传则移到根目录。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "doc_id": { "type": "number", "description": "文档ID" },
                "target_folder_id": { "type": "number", "description": "目标目录ID（不传=根目录）" }
            },
            "required": ["doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        let body = json!({ "target_folder_id": args["target_folder_id"] });
        match self
            .api
            .ownerkey_post(
                &format!("{DOCS_PREFIX}/{doc_id}/move"),
                &self.owner_key,
                &body,
            )
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "moved": true, "detail": resp }))),
            Err(e) => Ok(err_result(format!("移动文档失败: {e}"))),
        }
    }
}

// ── hx_doc_share ─────────────────────────────────────

pub struct HxDocShare {
    api: ApiClient,
    owner_key: String,
}

impl HxDocShare {
    pub fn new(api: ApiClient, owner_key: String) -> Self {
        Self { api, owner_key }
    }
}

#[async_trait]
impl Tool for HxDocShare {
    fn name(&self) -> &str {
        "hx_doc_share"
    }
    fn description(&self) -> &str {
        "生成文档分享链接。默认24小时有效期，只读权限。返回分享URL。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "doc_id": { "type": "number", "description": "文档ID" },
                "expires_hours": { "type": "number", "description": "有效期（小时），默认24" },
                "permission": { "type": "string", "description": "权限：view / edit，默认 view" }
            },
            "required": ["doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        let permission = args["permission"].as_str().unwrap_or("view");
        let expires = args["expires_hours"].as_i64().unwrap_or(24);
        let path =
            format!("{DOCS_PREFIX}/{doc_id}/share?permission={permission}&expires_hours={expires}");
        match self
            .api
            .ownerkey_post(&path, &self.owner_key, &json!({}))
            .await
        {
            Ok(resp) => Ok(ok_result(json!({ "share": resp }))),
            Err(e) => Ok(err_result(format!("生成分享链接失败: {e}"))),
        }
    }
}

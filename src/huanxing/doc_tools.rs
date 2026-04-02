//! HuanXing document management tools (Phase 3).
//!
//! 11 tools for folder + document CRUD + sharing.
//! All calls go through the backend API with X-User-Id header.

use crate::huanxing::api_client::ApiClient;
use crate::huanxing::db::TenantDb;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const DOCS_PREFIX: &str = "/api/v1/huanxing/agent/docs";

/// Extract phone number from agent_id format "001-18611348367-finance".
fn extract_phone(agent_id: &str) -> Option<&str> {
    let parts: Vec<&str> = agent_id.splitn(3, '-').collect();
    if parts.len() >= 2 && parts[1].len() == 11 && parts[1].chars().all(|c| c.is_ascii_digit()) {
        Some(parts[1])
    } else {
        None
    }
}

/// Resolve user_id (UUID) from agent_id by looking up in local DB.
async fn resolve_user_id(db: &TenantDb, agent_id: &str) -> Result<String, String> {
    // Try by agent_id directly
    if let Ok(Some(user)) = db.find_by_agent_id(agent_id).await {
        return Ok(user.user_id);
    }
    // Try extracting phone
    if let Some(phone) = extract_phone(agent_id) {
        if let Ok(Some(user)) = db.find_by_phone(phone).await {
            return Ok(user.user_id);
        }
    }
    Err(format!("无法确定用户身份 (agent_id={agent_id})"))
}

/// Shared context for all document tools.
#[derive(Clone)]
pub struct DocToolCtx {
    pub api: ApiClient,
    pub db: TenantDb,
    /// The agent_id of the calling agent (set at tool creation, used to resolve user).
    /// If empty, will need to be passed per-call.
    pub default_agent_id: Option<String>,
}

// ═══════════════════════════════════════════════════════
// Folder Tools (4)
// ═══════════════════════════════════════════════════════

// ── hx_folder_tree ───────────────────────────────────

pub struct HxFolderTree {
    api: ApiClient,
    db: TenantDb,
}

impl HxFolderTree {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
            "properties": {
                "agent_id": { "type": "string", "description": "Agent ID（用于确定用户身份）" }
            },
            "required": ["agent_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        match self
            .api
            .agent_get_as_user(&format!("{DOCS_PREFIX}/folders"), &[], &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "tree": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("获取目录树失败: {e}")),
            }),
        }
    }
}

// ── hx_folder_create ─────────────────────────────────

pub struct HxFolderCreate {
    api: ApiClient,
    db: TenantDb,
}

impl HxFolderCreate {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "name": { "type": "string", "description": "目录名称" },
                "parent_id": { "type": "number", "description": "父目录ID（不传则创建在根目录）" },
                "icon": { "type": "string", "description": "目录图标（emoji）" },
                "description": { "type": "string", "description": "目录描述" }
            },
            "required": ["agent_id", "name"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let mut body = json!({
            "name": args["name"],
        });
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
            .agent_post_as_user(&format!("{DOCS_PREFIX}/folders"), &body, &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "folder": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("创建目录失败: {e}")),
            }),
        }
    }
}

// ── hx_folder_delete ─────────────────────────────────

pub struct HxFolderDelete {
    api: ApiClient,
    db: TenantDb,
}

impl HxFolderDelete {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "folder_id": { "type": "number", "description": "目录ID" },
                "recursive": { "type": "boolean", "description": "是否递归删除（默认 false）" }
            },
            "required": ["agent_id", "folder_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let folder_id = args["folder_id"].as_i64().unwrap_or(0);
        let recursive = args["recursive"].as_bool().unwrap_or(false);
        let qs = if recursive { "?recursive=true" } else { "" };
        match self
            .api
            .agent_delete_as_user(&format!("{DOCS_PREFIX}/folders/{folder_id}{qs}"), &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "deleted": true, "detail": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("删除目录失败: {e}")),
            }),
        }
    }
}

// ── hx_folder_move ───────────────────────────────────

pub struct HxFolderMove {
    api: ApiClient,
    db: TenantDb,
}

impl HxFolderMove {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "folder_id": { "type": "number", "description": "要移动的目录ID" },
                "target_parent_id": { "type": "number", "description": "目标父目录ID（不传=根目录）" }
            },
            "required": ["agent_id", "folder_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let folder_id = args["folder_id"].as_i64().unwrap_or(0);
        let body = json!({ "target_parent_id": args["target_parent_id"] });
        match self
            .api
            .agent_post_as_user(
                &format!("{DOCS_PREFIX}/folders/{folder_id}/move"),
                &body,
                &user_id,
            )
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "moved": true, "detail": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("移动目录失败: {e}")),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════
// Document Tools (7)
// ═══════════════════════════════════════════════════════

// ── hx_doc_list ──────────────────────────────────────

pub struct HxDocList {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocList {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "folder_id": { "type": "number", "description": "目录ID（不传=根目录下的文档）" }
            },
            "required": ["agent_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let mut params: Vec<(&str, &str)> = Vec::new();
        let folder_str;
        if let Some(fid) = args["folder_id"].as_i64() {
            folder_str = fid.to_string();
            params.push(("folder_id", &folder_str));
        }
        match self
            .api
            .agent_get_as_user(DOCS_PREFIX, &params, &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "documents": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("获取文档列表失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_get ───────────────────────────────────────

pub struct HxDocGet {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocGet {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "doc_id": { "type": "number", "description": "文档ID" }
            },
            "required": ["agent_id", "doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        match self
            .api
            .agent_get_as_user(&format!("{DOCS_PREFIX}/{doc_id}"), &[], &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "document": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("获取文档失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_create ────────────────────────────────────

pub struct HxDocCreate {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocCreate {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "title": { "type": "string", "description": "文档标题" },
                "content": { "type": "string", "description": "文档内容（Markdown 格式）" },
                "tags": { "type": "string", "description": "标签（逗号分隔）" },
                "folder_id": { "type": "number", "description": "目录ID" },
                "status": { "type": "string", "description": "状态：draft / published / archived" }
            },
            "required": ["agent_id", "title", "content"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
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
            .agent_post_as_user(DOCS_PREFIX, &body, &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "document": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("创建文档失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_update ────────────────────────────────────

pub struct HxDocUpdate {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocUpdate {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "doc_id": { "type": "number", "description": "文档ID" },
                "title": { "type": "string", "description": "新标题" },
                "content": { "type": "string", "description": "新内容（Markdown 格式）" },
                "tags": { "type": "string", "description": "标签（逗号分隔）" },
                "status": { "type": "string", "description": "状态：draft / published / archived" }
            },
            "required": ["agent_id", "doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
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
            .agent_put_as_user(&format!("{DOCS_PREFIX}/{doc_id}"), &body, &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "updated": true, "detail": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("更新文档失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_delete ────────────────────────────────────

pub struct HxDocDelete {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocDelete {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "doc_id": { "type": "number", "description": "文档ID" }
            },
            "required": ["agent_id", "doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        match self
            .api
            .agent_delete_as_user(&format!("{DOCS_PREFIX}/{doc_id}"), &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "deleted": true, "detail": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("删除文档失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_move ──────────────────────────────────────

pub struct HxDocMove {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocMove {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "doc_id": { "type": "number", "description": "文档ID" },
                "target_folder_id": { "type": "number", "description": "目标目录ID（不传=根目录）" }
            },
            "required": ["agent_id", "doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        let body = json!({ "target_folder_id": args["target_folder_id"] });
        match self
            .api
            .agent_post_as_user(&format!("{DOCS_PREFIX}/{doc_id}/move"), &body, &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "moved": true, "detail": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("移动文档失败: {e}")),
            }),
        }
    }
}

// ── hx_doc_share ─────────────────────────────────────

pub struct HxDocShare {
    api: ApiClient,
    db: TenantDb,
}

impl HxDocShare {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
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
                "agent_id": { "type": "string", "description": "Agent ID" },
                "doc_id": { "type": "number", "description": "文档ID" },
                "expires_hours": { "type": "number", "description": "有效期（小时），默认24" },
                "permission": { "type": "string", "description": "权限：view / edit，默认 view" }
            },
            "required": ["agent_id", "doc_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let user_id = match resolve_user_id(&self.db, agent_id).await {
            Ok(uid) => uid,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };
        let doc_id = args["doc_id"].as_i64().unwrap_or(0);
        let permission = args["permission"].as_str().unwrap_or("view");
        let expires = args["expires_hours"].as_i64().unwrap_or(24);
        let path =
            format!("{DOCS_PREFIX}/{doc_id}/share?permission={permission}&expires_hours={expires}");
        match self
            .api
            .agent_post_as_user(&path, &json!({}), &user_id)
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({ "share": resp }).to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("生成分享链接失败: {e}")),
            }),
        }
    }
}

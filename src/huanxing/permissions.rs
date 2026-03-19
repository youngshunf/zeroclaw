//! Permission checks for HuanXing tool calls.
//!
//! Determines which tools are available based on agent context:
//! - **Guardian** can use admin/registration tools
//! - **Tenant agents** can only use self-service tools
//! - Some tools require the caller to be the data owner

/// Tools that only the Guardian agent may call.
const GUARDIAN_ONLY: &[&str] = &[
    "hx_register_user",
    "hx_create_agent",
    "hx_delete_agent",
    "hx_invalidate_cache",
    "hx_dashboard",
    "hx_local_stats",
    "hx_local_list_users",
    "hx_local_update_user",
    "hx_reload_gateway",
    "hx_backup_user",
];

/// Tools that tenant agents may call (self-service).
const TENANT_TOOLS: &[&str] = &[
    "hx_lookup_sender",
    "hx_get_user",
    "hx_check_quota",
    "hx_get_subscription",
    "hx_usage_stats",
    "hx_local_find_user",
    "hx_local_bind_channel",
    "hx_send_sms",
    "hx_verify_sms",
    // Document tools
    "hx_folder_tree",
    "hx_folder_create",
    "hx_folder_delete",
    "hx_folder_move",
    "hx_doc_list",
    "hx_doc_get",
    "hx_doc_create",
    "hx_doc_update",
    "hx_doc_delete",
    "hx_doc_move",
    "hx_doc_share",
    // HASN social
    "hasn_send",
    "hasn_contacts",
    "hasn_add_friend",
    "hasn_inbox",
    "hasn_respond_request",
    // Skill marketplace
    "hx_skill_search",
    "hx_skill_info",
    "hx_skill_install",
    "hx_skill_uninstall",
    "hx_skill_list",
    "hx_skill_update",
];

/// Check if a tool is guardian-only.
pub fn is_guardian_only(tool_name: &str) -> bool {
    GUARDIAN_ONLY.contains(&tool_name)
}

/// Check if an agent is the guardian.
pub fn is_guardian(agent_id: &str) -> bool {
    agent_id == "guardian"
        || agent_id == "huanxing-guardian"
        || agent_id.starts_with("guardian-")
}

/// Check if an agent is the admin.
pub fn is_admin(agent_id: &str) -> bool {
    agent_id == "admin"
        || agent_id == "huanxing-admin"
        || agent_id.starts_with("admin-")
}

/// Check if a tool is available to a tenant agent.
pub fn is_tenant_tool(tool_name: &str) -> bool {
    TENANT_TOOLS.contains(&tool_name)
}

/// Verify that the given agent has permission to call the tool.
/// Returns Ok(()) if allowed, Err with reason if denied.
pub fn check_permission(agent_id: &str, tool_name: &str) -> Result<(), String> {
    // Guardian and Admin can use everything
    if is_guardian(agent_id) || is_admin(agent_id) {
        return Ok(());
    }

    // Tenant agents cannot use guardian-only tools
    if is_guardian_only(tool_name) {
        return Err(format!(
            "工具 {tool_name} 仅限 Guardian 使用，当前 Agent: {agent_id}"
        ));
    }

    // All other tools are allowed for tenant agents
    Ok(())
}

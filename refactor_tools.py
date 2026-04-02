import os
import re

files = [
    "src/huanxing/hasn_tools.rs",
    "src/huanxing/skill_market_tools.rs",
    "src/huanxing/secret_tools.rs"
]

for f in files:
    with open(f, "r") as f_in:
        content = f_in.read()

    # Import TenantDb
    if "crate::huanxing::db::TenantDb" not in content and "TenantDb" not in content:
        content = content.replace("use std::path::PathBuf;", "use std::path::PathBuf;\nuse crate::huanxing::db::TenantDb;")
    elif "use std::path::PathBuf;" in content:
        content = content.replace("use std::path::PathBuf;", "use std::path::PathBuf;\nuse crate::huanxing::db::TenantDb;")

    # Fix struct definitions
    content = re.sub(r'agents_dir:\s*PathBuf,', 'config_dir: PathBuf,\n    db: TenantDb,', content)
    content = re.sub(r'workspace_dir:\s*PathBuf,', 'config_dir: PathBuf,\n    db: TenantDb,', content)

    # Note: SecretTools and SkillTools used `pub workspace_dir: PathBuf,`
    content = re.sub(r'pub workspace_dir:\s*PathBuf,', 'pub config_dir: PathBuf,\n    pub db: TenantDb,', content)

    # In hasn_tools.rs the fn is:
    # fn resolve_workspace(agents_dir: &std::path::Path, agent_id: &str) -> PathBuf {
    #     agents_dir.join(agent_id)
    # }
    if "fn resolve_workspace" in content and "agents_dir.join" in content:
        content = content.replace("fn resolve_workspace(agents_dir: &std::path::Path, agent_id: &str) -> PathBuf {", 
                                  "async fn resolve_workspace(db: &TenantDb, config_dir: &std::path::Path, agent_id: &str) -> PathBuf {")
        content = content.replace("    agents_dir.join(agent_id)",
                                  """    if let Ok(Some(user)) = db.find_by_agent_id(agent_id).await {
        return config_dir.join("users").join(user.user_id).join("agents").join(agent_id).join("workspace");
    }
    // Fallback if not found for legacy test cases
    config_dir.join("admin").join("agents").join(agent_id).join("workspace")""")
    else:
        # Add resolver if it doesn't have one
        if "async fn resolve_workspace" not in content:
            resolver = """
async fn resolve_workspace(db: &TenantDb, config_dir: &std::path::Path, agent_id: &str) -> PathBuf {
    if let Ok(Some(user)) = db.find_by_agent_id(agent_id).await {
        return config_dir.join("users").join(user.user_id).join("agents").join(agent_id).join("workspace");
    }
    config_dir.join("admin").join("agents").join(agent_id).join("workspace")
}
"""
            content = content.replace("use crate::huanxing::db::TenantDb;", "use crate::huanxing::db::TenantDb;\n" + resolver)

    # Call sites for resolve_workspace
    content = content.replace("resolve_workspace(&self.agents_dir, agent_id)", "resolve_workspace(&self.db, &self.config_dir, agent_id).await")
    content = content.replace("resolve_workspace(&self.workspace_dir, agent_id)", "resolve_workspace(&self.db, &self.config_dir, agent_id).await")

    # Fix Hasn tools new() method declarations
    content = re.sub(r'agents_dir:\s*PathBuf,', 'config_dir: PathBuf, db: TenantDb,', content)
    # Actually we just regex for the `pub fn new(` and update its arguments
    content = re.sub(r'agents_dir:\s*PathBuf', 'config_dir: PathBuf, db: TenantDb', content)
    content = re.sub(r'agents_dir,', 'config_dir,\n            db,', content)

    with open(f, "w") as f_out:
        f_out.write(content)

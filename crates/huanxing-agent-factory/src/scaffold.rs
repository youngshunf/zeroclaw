pub struct EmbeddedScaffold {
    pub name: &'static str,
    pub content: &'static str,
}

pub fn global_scaffold() -> Vec<EmbeddedScaffold> {
    vec![
        EmbeddedScaffold { name: "config.toml.template", content: include_str!("scaffold/global/config.toml.template") },
    ]
}

pub fn owner_scaffold() -> Vec<EmbeddedScaffold> {
    vec![
        EmbeddedScaffold { name: "USER.md", content: include_str!("scaffold/owner/USER.md") },
        EmbeddedScaffold { name: "MEMORY.md", content: include_str!("scaffold/owner/MEMORY.md") },
        EmbeddedScaffold { name: "BOOTSTRAP.md", content: include_str!("scaffold/owner/BOOTSTRAP.md") },
        EmbeddedScaffold { name: "TASK_LEDGER.md", content: include_str!("scaffold/owner/TASK_LEDGER.md") },
        EmbeddedScaffold { name: "HEARTBEAT.md", content: include_str!("scaffold/owner/HEARTBEAT.md") },
        EmbeddedScaffold { name: "config.toml.template", content: include_str!("scaffold/owner/config.toml.template") },
    ]
}

pub fn agent_scaffold() -> Vec<EmbeddedScaffold> {
    vec![
        EmbeddedScaffold { name: "IDENTITY.md", content: include_str!("scaffold/agent/IDENTITY.md") },
        EmbeddedScaffold { name: "SOUL.md", content: include_str!("scaffold/agent/SOUL.md") },
        EmbeddedScaffold { name: "AGENTS.md", content: include_str!("scaffold/agent/AGENTS.md") },
        EmbeddedScaffold { name: "TOOLS.md", content: include_str!("scaffold/agent/TOOLS.md") },
        EmbeddedScaffold { name: "config.toml.template", content: include_str!("scaffold/agent/config.toml.template") },
    ]
}

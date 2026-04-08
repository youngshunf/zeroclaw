pub enum EmbeddedContent {
    Text(&'static str),
    Binary(&'static [u8]),
}

pub struct EmbeddedScaffold {
    pub name: &'static str,
    pub content: EmbeddedContent,
}

pub fn global_scaffold() -> Vec<EmbeddedScaffold> {
    vec![EmbeddedScaffold {
        name: "config.toml.template",
        content: EmbeddedContent::Text(include_str!("scaffold/global/config.toml.template")),
    }]
}

pub fn owner_scaffold() -> Vec<EmbeddedScaffold> {
    vec![
        EmbeddedScaffold {
            name: "USER.md",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/USER.md")),
        },
        EmbeddedScaffold {
            name: "MEMORY.md",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/MEMORY.md")),
        },
        EmbeddedScaffold {
            name: "BOOTSTRAP.md",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/BOOTSTRAP.md")),
        },
        EmbeddedScaffold {
            name: "TASK_LEDGER.md",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/TASK_LEDGER.md")),
        },
        EmbeddedScaffold {
            name: "HEARTBEAT.md",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/HEARTBEAT.md")),
        },
        EmbeddedScaffold {
            name: "config.toml.template",
            content: EmbeddedContent::Text(include_str!("scaffold/owner/config.toml.template")),
        },
    ]
}

pub fn agent_scaffold() -> Vec<EmbeddedScaffold> {
    vec![
        EmbeddedScaffold {
            name: "IDENTITY.md",
            content: EmbeddedContent::Text(include_str!("scaffold/agent/IDENTITY.md")),
        },
        EmbeddedScaffold {
            name: "SOUL.md",
            content: EmbeddedContent::Text(include_str!("scaffold/agent/SOUL.md")),
        },
        EmbeddedScaffold {
            name: "AGENTS.md",
            content: EmbeddedContent::Text(include_str!("scaffold/agent/AGENTS.md")),
        },
        EmbeddedScaffold {
            name: "TOOLS.md",
            content: EmbeddedContent::Text(include_str!("scaffold/agent/TOOLS.md")),
        },
        EmbeddedScaffold {
            name: "config.toml.template",
            content: EmbeddedContent::Text(include_str!("scaffold/agent/config.toml.template")),
        },
        EmbeddedScaffold {
            name: "icon.png",
            content: EmbeddedContent::Binary(include_bytes!("scaffold/agent/icon.png")),
        },
    ]
}

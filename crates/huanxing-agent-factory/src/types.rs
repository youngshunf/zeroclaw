use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_skills")]
    pub skills: SkillsConfig,

    // We only need the ID array from marketplace to do downloads
    #[serde(default)]
    pub sops: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default)]
    pub exclusive: Vec<String>,
}

fn deserialize_skills<'de, D>(deserializer: D) -> std::result::Result<SkillsConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SkillsRaw {
        Flat(Vec<String>),
        Structured(SkillsConfig),
    }

    match SkillsRaw::deserialize(deserializer) {
        Ok(SkillsRaw::Flat(list)) => Ok(SkillsConfig { exclusive: list }),
        Ok(SkillsRaw::Structured(cfg)) => Ok(cfg),
        Err(_) => Ok(SkillsConfig::default()),
    }
}

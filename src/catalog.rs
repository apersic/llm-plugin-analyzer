use crate::domain::DocRef;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginRef {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub plugin_root: PathBuf,
    pub plugin_name: String,
    pub skills: BTreeSet<String>,
    pub principles: BTreeSet<String>,
    pub playbooks: BTreeSet<String>,
    pub playbook_mentions: BTreeMap<String, BTreeSet<String>>,
}

impl Catalog {
    pub fn load(plugin_root: PathBuf) -> anyhow::Result<Self> {
        let meta = read_plugin_meta(&plugin_root);
        let mut cat = Self {
            plugin_root: plugin_root.clone(),
            plugin_name: meta
                .as_ref()
                .map(|m| m.display())
                .unwrap_or_else(|| folder_name(&plugin_root)),
            ..Self::default()
        };
        let skills = plugin_root.join("skills");
        if !skills.is_dir() {
            anyhow::bail!("plugin skills dir missing: {}", skills.display());
        }
        for entry in WalkDir::new(&skills).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
                continue;
            }
            if let Some(name) = skill_dir_name(path, &skills) {
                if let Some(rest) = name.strip_prefix("principle-") {
                    cat.principles.insert(rest.to_string());
                } else {
                    cat.skills.insert(name);
                }
            }
        }
        for entry in WalkDir::new(&skills).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let is_playbook = path.components().any(|c| c.as_os_str() == "playbooks");
            if !is_playbook {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            cat.playbooks.insert(stem.to_string());
            let text = std::fs::read_to_string(path).unwrap_or_default();
            cat.playbook_mentions
                .insert(stem.to_string(), mentioned_skills(&text, &cat.skills));
        }
        Ok(cat)
    }

    pub fn classify(&self, path: &str) -> Option<DocRef> {
        let norm = path.replace('\\', "/").to_ascii_lowercase();
        if let Some(rest) = after(&norm, "/playbooks/") {
            let stem = rest.trim_end_matches(".md");
            if self.playbooks.contains(stem) {
                return Some(DocRef::Playbook(stem.to_string()));
            }
        }
        if let Some(rest) = after(&norm, "/skills/") {
            if let Some((dir, file)) = rest.split_once('/') {
                if file.ends_with("/skill.md") || file == "skill.md" {
                    if let Some(p) = dir.strip_prefix("principle-") {
                        if self.principles.contains(p) {
                            return Some(DocRef::Principle(p.to_string()));
                        }
                    }
                    if self.skills.contains(dir) {
                        return Some(DocRef::Skill(dir.to_string()));
                    }
                }
                if file.contains("/references/") || file.starts_with("references/") {
                    let skill = dir.to_string();
                    if self.skills.contains(&skill) || skill.starts_with("principle-") {
                        return Some(DocRef::Reference {
                            skill,
                            file: file.to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    pub fn digest(&self) -> u64 {
        let mut acc = hash64(self.plugin_root.to_string_lossy().as_bytes());
        for s in &self.skills {
            acc ^= hash64(s.as_bytes());
        }
        for s in &self.playbooks {
            acc = acc.wrapping_add(hash64(s.as_bytes()));
        }
        acc
    }
}

pub fn discover(cursor_home: &Path) -> Vec<PluginRef> {
    let plugins = cursor_home.join("plugins");
    if !plugins.is_dir() {
        return Vec::new();
    }
    let mut found: BTreeMap<String, PluginRef> = BTreeMap::new();
    for entry in WalkDir::new(&plugins)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        if !is_plugin_root(path) {
            continue;
        }
        let plugin = plugin_from_dir(path);
        let local = path.components().any(|c| c.as_os_str() == "local");
        match found.get(&plugin.id) {
            Some(existing) if path_is_local(&existing.root) => {}
            _ => {
                if local || !found.contains_key(&plugin.id) {
                    found.insert(plugin.id.clone(), plugin);
                }
            }
        }
    }
    found.into_values().collect()
}

fn path_is_local(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "local")
}

fn is_plugin_root(path: &Path) -> bool {
    let skills = path.join("skills");
    if !skills.is_dir() {
        return false;
    }
    WalkDir::new(&skills)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name() == "SKILL.md")
}

fn plugin_from_dir(path: &Path) -> PluginRef {
    let meta = read_plugin_meta(path);
    let id = meta
        .as_ref()
        .and_then(|m| m.name.clone())
        .unwrap_or_else(|| folder_name(path));
    let name = meta
        .as_ref()
        .map(|m| m.display())
        .unwrap_or_else(|| id.clone());
    PluginRef {
        id,
        name,
        root: path.to_path_buf(),
    }
}

#[derive(Deserialize)]
struct PluginMeta {
    name: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

impl PluginMeta {
    fn display(&self) -> String {
        self.display_name
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| "plugin".into())
    }
}

fn read_plugin_meta(root: &Path) -> Option<PluginMeta> {
    for rel in [".cursor-plugin/plugin.json", "plugin.json"] {
        let path = root.join(rel);
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Ok(meta) = serde_json::from_str::<PluginMeta>(&text) {
            return Some(meta);
        }
    }
    None
}

fn folder_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("plugin");
    if looks_like_hash(name) {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
            return parent.to_string_lossy().into_owned();
        }
    }
    name.to_string()
}

fn looks_like_hash(name: &str) -> bool {
    name.len() >= 32 && name.chars().all(|c| c.is_ascii_hexdigit())
}

fn skill_dir_name(skill_md: &Path, skills_root: &Path) -> Option<String> {
    let parent = skill_md.parent()?;
    let rel = parent.strip_prefix(skills_root).ok()?;
    rel.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

fn after<'a>(hay: &'a str, needle: &str) -> Option<&'a str> {
    hay.rsplit_once(needle).map(|(_, rest)| rest)
}

fn mentioned_skills(text: &str, skills: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for skill in skills {
        if text.contains(skill) {
            out.insert(skill.clone());
        }
    }
    out
}

pub fn hash64(bytes: &[u8]) -> u64 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 prefix"))
}

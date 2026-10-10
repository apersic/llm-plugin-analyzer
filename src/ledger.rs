use crate::catalog::{hash64, Catalog};
use crate::domain::{
    Event, EventBody, EventId, ModelRef, Query, SessionId, ThreadId, TodoItem, ToolCall,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub events: Vec<Event>,
    pub digest: u64,
}

impl Snapshot {
    pub fn scoped<'a>(&'a self, query: &'a Query) -> impl Iterator<Item = &'a Event> {
        self.events.iter().filter(move |e| query.scope.matches(e))
    }
}

#[derive(Clone, Debug)]
struct CachedFile {
    stamp: u64,
    events: Vec<Event>,
}

#[derive(Clone, Debug, Default)]
pub struct Scanner {
    files: BTreeMap<PathBuf, CachedFile>,
    last_stamp: u64,
    last: Option<Snapshot>,
}

impl Scanner {
    pub fn disk_stamp(cursor_home: &Path) -> u64 {
        list_jsonl(cursor_home)
            .into_iter()
            .fold(0u64, |acc, path| acc ^ file_stamp(&path))
    }

    pub fn refresh(&mut self, cursor_home: &Path, catalog: &Catalog) -> anyhow::Result<Snapshot> {
        let stamp = Self::disk_stamp(cursor_home);
        if stamp == self.last_stamp {
            if let Some(snap) = &self.last {
                return Ok(snap.clone());
            }
        }
        let paths = list_jsonl(cursor_home);
        let live: BTreeMap<PathBuf, ()> = paths.iter().cloned().map(|p| (p, ())).collect();
        self.files.retain(|p, _| live.contains_key(p));
        for path in &paths {
            let stamp = file_stamp(path);
            if self.files.get(path).is_some_and(|c| c.stamp == stamp) {
                continue;
            }
            let mut events = Vec::new();
            ingest_file(path, catalog, &mut events)?;
            self.files
                .insert(path.clone(), CachedFile { stamp, events });
        }
        let mut events = Vec::new();
        for path in &paths {
            if let Some(cached) = self.files.get(path) {
                events.extend(cached.events.iter().cloned());
            }
        }
        let digest = events.iter().fold(catalog.digest(), |acc, e| {
            acc.wrapping_add(e.id.source ^ u64::from(e.id.line))
        });
        let snap = Snapshot { events, digest };
        self.last_stamp = stamp;
        self.last = Some(snap.clone());
        Ok(snap)
    }
}

pub fn scan_once(cursor_home: &Path, catalog: &Catalog) -> anyhow::Result<Snapshot> {
    Scanner::default().refresh(cursor_home, catalog)
}

fn list_jsonl(cursor_home: &Path) -> Vec<PathBuf> {
    let projects = cursor_home.join("projects");
    if !projects.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = WalkDir::new(&projects)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .filter(|p| p.components().any(|c| c.as_os_str() == "agent-transcripts"))
        .collect();
    files.sort();
    files
}

fn file_stamp(path: &Path) -> u64 {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return hash64(path.to_string_lossy().as_bytes()),
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    hash64(path.to_string_lossy().as_bytes()) ^ mtime ^ meta.len()
}

fn ingest_file(path: &Path, catalog: &Catalog, events: &mut Vec<Event>) -> anyhow::Result<()> {
    let Some((workspace, conversation, child)) = locate(path) else {
        return Ok(());
    };
    let session = SessionId {
        workspace,
        conversation: conversation.clone(),
    };
    let thread = match child {
        Some(id) => ThreadId::Child(session, id),
        None => ThreadId::Root(session),
    };
    let source = hash64(path.to_string_lossy().as_bytes());
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut stamp: Option<String> = None;
    for (i, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<WireLine>(&line) else {
            continue;
        };
        if raw.ty.as_deref() == Some("turn_ended") {
            events.push(Event {
                id: EventId {
                    source,
                    line: i as u32,
                    part: 0,
                },
                thread: thread.clone(),
                stamp: stamp.clone(),
                body: EventBody::TurnEnded,
            });
            continue;
        }
        let Some(message) = raw.message else {
            continue;
        };
        let role = raw.role.unwrap_or_default();
        for (part, item) in message.content.into_iter().enumerate() {
            let id = EventId {
                source,
                line: i as u32,
                part: part as u16,
            };
            match item {
                WirePart {
                    ty,
                    text,
                    name,
                    input,
                } => {
                    if ty == "text" {
                        let text = text.unwrap_or_default();
                        if role == "user" {
                            if let Some(ts) = extract_timestamp(&text) {
                                stamp = Some(ts);
                            }
                            let slash = extract_slash(&text);
                            events.push(Event {
                                id,
                                thread: thread.clone(),
                                stamp: stamp.clone(),
                                body: EventBody::UserTurn {
                                    slash,
                                    text: text.into(),
                                },
                            });
                        } else if role == "assistant" {
                            events.push(Event {
                                id,
                                thread: thread.clone(),
                                stamp: stamp.clone(),
                                body: EventBody::AssistantText { text: text.into() },
                            });
                        }
                    } else if ty == "tool_use" {
                        let name = name.unwrap_or_default();
                        let body = classify_tool(&name, input.as_ref(), catalog);
                        events.push(Event {
                            id,
                            thread: thread.clone(),
                            stamp: stamp.clone(),
                            body: EventBody::Tool(body),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn classify_tool(name: &str, input: Option<&Value>, catalog: &Catalog) -> ToolCall {
    match name {
        "Read" => {
            let path = input
                .and_then(|v| v.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(doc) = catalog.classify(path) {
                ToolCall::ReadDoc(doc)
            } else {
                ToolCall::Other {
                    name: "Read".into(),
                }
            }
        }
        "Task" => {
            let model = input
                .and_then(|v| v.get("model"))
                .and_then(|v| v.as_str())
                .map(|s| ModelRef::Requested(s.to_string()))
                .unwrap_or(ModelRef::Inherited);
            let prompt = input
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
            let description = input
                .and_then(|v| v.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
            ToolCall::Spawn {
                model,
                prompt,
                description,
            }
        }
        "TodoWrite" => {
            let items = input
                .and_then(|v| v.get("todos"))
                .and_then(|v| v.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|row| row.get("content").and_then(|c| c.as_str()))
                        .map(|text| TodoItem {
                            skipped: text.starts_with("skip:"),
                            text: text.into(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            ToolCall::Todo { items }
        }
        "Write" | "StrReplace" => {
            let path = input
                .and_then(|v| v.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            ToolCall::Edit {
                path: PathBuf::from(path),
            }
        }
        "Shell" => {
            let cmd = input
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            ToolCall::Shell {
                command_head: cmd.chars().take(80).collect::<String>().into(),
            }
        }
        other => ToolCall::Other {
            name: other.to_string(),
        },
    }
}

fn locate(path: &Path) -> Option<(String, String, Option<String>)> {
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let projects = parts.iter().position(|p| p == "projects")?;
    let workspace = parts.get(projects + 1)?.clone();
    let transcripts = parts.iter().position(|p| p == "agent-transcripts")?;
    let conversation = parts.get(transcripts + 1)?.clone();
    let child = if parts.iter().any(|p| p == "subagents") {
        path.file_stem().map(|s| s.to_string_lossy().into_owned())
    } else {
        None
    };
    Some((workspace, conversation, child))
}

fn extract_timestamp(text: &str) -> Option<String> {
    let start = text.find("<timestamp>")? + "<timestamp>".len();
    let end = text[start..].find("</timestamp>")?;
    Some(text[start..start + end].trim().to_string())
}

fn extract_slash(text: &str) -> Vec<String> {
    let hay = text.split("<user_query>").nth(1).unwrap_or(text);
    hay.split_whitespace()
        .filter(|w| w.starts_with('/') && w.len() > 1)
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[derive(Deserialize)]
struct WireLine {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    message: Option<WireMessage>,
    #[serde(default, rename = "type")]
    ty: Option<String>,
}

#[derive(Deserialize)]
struct WireMessage {
    #[serde(default)]
    content: Vec<WirePart>,
}

#[derive(Deserialize)]
struct WirePart {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<Value>,
}

pub fn tool_histogram<'a, I>(events: I) -> BTreeMap<String, u32>
where
    I: IntoIterator<Item = &'a Event>,
{
    let mut map = BTreeMap::new();
    for e in events {
        if let EventBody::Tool(t) = &e.body {
            let name = match t {
                ToolCall::ReadDoc(_) => "ReadDoc",
                ToolCall::Spawn { .. } => "Task",
                ToolCall::Todo { .. } => "TodoWrite",
                ToolCall::Edit { .. } => "Edit",
                ToolCall::Shell { .. } => "Shell",
                ToolCall::Other { name } => name.as_str(),
            };
            *map.entry(name.to_string()).or_insert(0) += 1;
        }
    }
    map
}

use llm_plugin_analyzer::catalog::{self, Catalog};
use llm_plugin_analyzer::domain::{AdviceKind, DocRef, Query, Scope, Tier};
use llm_plugin_analyzer::ledger::{scan_once, Scanner};
use llm_plugin_analyzer::project;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn catalog() -> Catalog {
    let mut cat = Catalog::default();
    cat.skills.insert("how".into());
    cat.skills.insert("architect".into());
    cat.playbooks.insert("feature".into());
    cat.playbook_mentions
        .insert("feature".into(), BTreeSet::from(["how".into()]));
    cat
}

fn write_jsonl(root: &Path, workspace: &str, conversation: &str, lines: &str) -> PathBuf {
    let dir = root
        .join("projects")
        .join(workspace)
        .join("agent-transcripts")
        .join(conversation);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("session.jsonl");
    fs::write(&path, lines).unwrap();
    path
}

fn skill_path(name: &str) -> String {
    format!("C:/Users/Nax/.cursor/plugins/local/guide-to-the-code/skills/{name}/SKILL.md")
}

fn playbook_path(name: &str) -> String {
    format!(
        "C:/Users/Nax/.cursor/plugins/local/guide-to-the-code/skills/nax-mode/playbooks/{name}.md"
    )
}

fn tool(name: &str, input: &str) -> String {
    format!(
        r#"{{"role":"assistant","message":{{"content":[{{"type":"tool_use","name":"{name}","input":{input}}}]}}}}"#
    )
}

fn user(text: &str) -> String {
    format!(
        r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>{text}</user_query>"}}]}}}}"#
    )
}

fn assistant(text: &str) -> String {
    format!(r#"{{"role":"assistant","message":{{"content":[{{"type":"text","text":"{text}"}}]}}}}"#)
}

fn join(lines: &[&str]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[test]
fn replay_same_bytes_same_digest() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "conv",
        &join(&[
            &user("/nax-mode ship it"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let a = scan_once(tmp.path(), &cat).unwrap();
    let b = scan_once(tmp.path(), &cat).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.events.len(), b.events.len());
    assert_eq!(a.events.len(), 3);
}

#[test]
fn read_then_todo_is_hit() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "hit",
        &join(&[
            &user("/nax-mode"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            &tool(
                "TodoWrite",
                r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
            ),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "how")
        .expect("how node");
    assert_eq!(node.tally.hit, 1);
    assert_eq!(node.tally.miss, 0);
}

#[test]
fn cited_without_read_is_miss() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "miss-cite",
        &join(&[
            &user("go"),
            &assistant("I will follow `how` without opening it"),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "how")
        .expect("how node");
    assert_eq!(node.tally.miss, 1);
}

#[test]
fn read_never_used_after_settle_is_miss() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "miss-read",
        &join(&[
            &user("/nax-mode"),
            &tool(
                "Read",
                &format!(r#"{{"path":"{}"}}"#, skill_path("architect")),
            ),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "architect")
        .expect("architect node");
    assert_eq!(node.tally.miss, 1);
}

#[test]
fn unread_open_thread_stays_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "open",
        &join(&[
            &user("/nax-mode"),
            &tool(
                "Read",
                &format!(r#"{{"path":"{}"}}"#, skill_path("architect")),
            ),
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "architect")
        .expect("architect node");
    assert_eq!(node.tally.unknown, 1);
    assert_eq!(node.tally.miss, 0);
}

#[test]
fn playbook_then_mentioned_skill_draws_edge() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "route",
        &join(&[
            &user("/nax-mode"),
            &tool(
                "Read",
                &format!(r#"{{"path":"{}"}}"#, playbook_path("feature")),
            ),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    assert_eq!(board.routing_expected, 1);
    assert_eq!(board.routing_hits, 1);
    assert!(!board.edges.is_empty());
}

#[test]
fn classify_plugin_paths() {
    let cat = catalog();
    assert_eq!(
        cat.classify(&skill_path("how")),
        Some(DocRef::Skill("how".into()))
    );
    assert_eq!(
        cat.classify(&playbook_path("feature")),
        Some(DocRef::Playbook("feature".into()))
    );
    assert_eq!(cat.classify("C:/tmp/unrelated.rs"), None);
}

#[test]
fn scanner_skips_unchanged_files() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "cache",
        &join(&[
            &user("/nax-mode"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let mut scanner = Scanner::default();
    let a = scanner.refresh(tmp.path(), &cat).unwrap();
    let b = scanner.refresh(tmp.path(), &cat).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.events.len(), b.events.len());
}

#[test]
fn two_threads_count_separately() {
    let tmp = tempfile::tempdir().unwrap();
    let body = join(&[
        &user("/nax-mode"),
        &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
        &tool(
            "TodoWrite",
            r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
        ),
        r#"{"type":"turn_ended"}"#,
    ]);
    write_jsonl(tmp.path(), "ws", "one", &body);
    write_jsonl(tmp.path(), "ws", "two", &body);
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "how")
        .expect("how node");
    assert_eq!(node.tally.hit, 2);
    assert_eq!(node.tally.contacts, 2);
}

#[test]
fn inspect_missing_node() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(tmp.path(), "ws", "empty", &join(&[&user("hi")]));
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    assert!(project::inspect(&board, "nope").is_none());
}

#[test]
fn inspect_columns_from_tally() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "insp",
        &join(&[
            &user("/nax-mode"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            &tool(
                "TodoWrite",
                r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
            ),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let info = project::inspect(&board, "how").expect("how");
    assert_eq!(info.lane, "Skills");
    assert_eq!(info.hit, 1);
    assert_eq!(info.miss, 0);
    assert_eq!(info.contacts, 1);
}

#[test]
fn open_thread_emits_flight_between_reads() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "fly",
        &join(&[
            &user("/nax-mode"),
            &tool(
                "Read",
                &format!(r#"{{"path":"{}"}}"#, playbook_path("feature")),
            ),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    assert_eq!(board.flights.len(), 1);
    assert_eq!(
        board.flights[0].stops,
        vec!["chat".to_string(), "feature".into(), "how".into()]
    );
    assert!(board.live);
}

#[test]
fn settled_thread_replays_latest_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "done",
        &join(&[
            &user("/nax-mode"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    assert_eq!(board.flights.len(), 1);
    assert_eq!(
        board.flights[0].stops,
        vec!["chat".to_string(), "how".into()]
    );
    assert_eq!(board.prompt_preview.as_deref(), Some("/nax-mode"));
    assert_eq!(board.visits, 1);
    assert!(!board.live);
    let chat = board
        .nodes
        .values()
        .find(|n| n.label == "chat")
        .expect("chat node");
    assert_eq!(chat.tally.contacts, 1);
}

#[test]
fn chat_scope_counts_one_thread() {
    let tmp = tempfile::tempdir().unwrap();
    let body = join(&[
        &user("/nax-mode"),
        &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
        &tool(
            "TodoWrite",
            r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
        ),
        r#"{"type":"turn_ended"}"#,
    ]);
    write_jsonl(tmp.path(), "ws", "one", &body);
    write_jsonl(tmp.path(), "ws", "two", &body);
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(
        &snap,
        &cat,
        &Query {
            scope: Scope::Chat {
                workspace: "ws".into(),
                conversation: "one".into(),
            },
        },
    );
    let node = board
        .nodes
        .values()
        .find(|n| n.label == "how")
        .expect("how node");
    assert_eq!(node.tally.hit, 1);
    assert_eq!(node.tally.contacts, 1);
}

#[test]
fn directory_lists_workspace_chats() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(tmp.path(), "ws", "alpha", &join(&[&user("first prompt")]));
    write_jsonl(tmp.path(), "ws", "beta", &join(&[&user("second prompt")]));
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let dir = project::directory(&snap);
    assert_eq!(dir.workspaces.len(), 1);
    assert_eq!(dir.workspaces[0].id, "ws");
    assert_eq!(dir.workspaces[0].chats.len(), 2);
    let labels: Vec<_> = dir.workspaces[0]
        .chats
        .iter()
        .map(|c| c.label.as_str())
        .collect();
    assert!(labels.contains(&"first prompt"));
    assert!(labels.contains(&"second prompt"));
}

#[test]
fn inspect_chat_shows_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "chat",
        &join(&[&user("ship the visualizer")]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let info = project::inspect(&board, "chat").expect("chat");
    assert_eq!(info.lane, "Chat");
    assert_eq!(info.detail.as_deref(), Some("ship the visualizer"));
    assert_eq!(info.contacts, 1);
}

#[test]
fn discover_reads_plugin_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plugins").join("local").join("demo-plugin");
    fs::create_dir_all(root.join(".cursor-plugin")).unwrap();
    fs::create_dir_all(root.join("skills").join("alpha")).unwrap();
    fs::write(
        root.join(".cursor-plugin").join("plugin.json"),
        r#"{"name":"demo-plugin","displayName":"Demo Plugin"}"#,
    )
    .unwrap();
    fs::write(
        root.join("skills").join("alpha").join("SKILL.md"),
        "# alpha\n",
    )
    .unwrap();
    let plugins = catalog::discover(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, "demo-plugin");
    assert_eq!(plugins[0].name, "Demo Plugin");
}

#[test]
fn discover_reads_root_plugin_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plugins").join("local").join("root-meta");
    fs::create_dir_all(root.join("skills").join("alpha")).unwrap();
    fs::write(
        root.join("plugin.json"),
        r#"{"name":"root-meta","displayName":"Root Meta"}"#,
    )
    .unwrap();
    fs::write(
        root.join("skills").join("alpha").join("SKILL.md"),
        "# alpha\n",
    )
    .unwrap();
    let plugins = catalog::discover(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "Root Meta");
}

#[test]
fn load_playbooks_from_any_skill_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let skills = tmp.path().join("skills");
    fs::create_dir_all(skills.join("alpha")).unwrap();
    fs::create_dir_all(skills.join("mode").join("playbooks")).unwrap();
    fs::write(skills.join("alpha").join("SKILL.md"), "# alpha\n").unwrap();
    fs::write(
        skills.join("mode").join("playbooks").join("ship.md"),
        "call alpha\n",
    )
    .unwrap();
    let cat = Catalog::load(tmp.path().to_path_buf()).unwrap();
    assert!(cat.skills.contains("alpha"));
    assert!(cat.playbooks.contains("ship"));
}

#[test]
fn advise_marks_unused_read_as_remove() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "miss-read",
        &join(&[
            &user("/nax-mode"),
            &tool(
                "Read",
                &format!(r#"{{"path":"{}"}}"#, skill_path("architect")),
            ),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let rows = project::advise(&board);
    let row = rows
        .iter()
        .find(|r| r.label == "architect")
        .expect("architect");
    assert_eq!(row.tier, Tier::F);
    assert!(matches!(row.kind, AdviceKind::Remove { .. }));
}

#[test]
fn advise_keeps_a_hit() {
    let tmp = tempfile::tempdir().unwrap();
    write_jsonl(
        tmp.path(),
        "ws",
        "hit",
        &join(&[
            &user("/nax-mode"),
            &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
            &tool(
                "TodoWrite",
                r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
            ),
            r#"{"type":"turn_ended"}"#,
        ]),
    );
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let board = project::board(&snap, &cat, &Query::all());
    let rows = project::advise(&board);
    let row = rows.iter().find(|r| r.label == "how").expect("how");
    assert!(matches!(row.kind, AdviceKind::Keep));
}

#[test]
fn inspect_series_counts_each_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let body = join(&[
        &user("/nax-mode"),
        &tool("Read", &format!(r#"{{"path":"{}"}}"#, skill_path("how"))),
        &tool(
            "TodoWrite",
            r#"{"todos":[{"content":"apply how","status":"in_progress"}]}"#,
        ),
        r#"{"type":"turn_ended"}"#,
    ]);
    write_jsonl(tmp.path(), "ws", "one", &body);
    write_jsonl(tmp.path(), "ws", "two", &body);
    let cat = catalog();
    let snap = scan_once(tmp.path(), &cat).unwrap();
    let series = project::inspect_series(&snap, &cat, &Query::all(), "how");
    assert_eq!(series.hits, vec![1, 1]);
    assert_eq!(series.misses, vec![0, 0]);
}

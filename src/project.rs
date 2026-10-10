use crate::catalog::Catalog;
use crate::domain::{
    Advice, AdviceKind, Board, DocRef, Edge, Event, EventBody, Flight, HitEvidence, InspectSeries,
    Inspection, Lane, MissReason, Node, NodeKey, Query, SessionId, Tally, ThreadId, Tier,
    TokenSpend, ToolCall, Verdict,
};
use crate::ledger::{tool_histogram, Snapshot};
use crate::theme;
use std::collections::{BTreeMap, BTreeSet};

pub fn board(snap: &Snapshot, catalog: &Catalog, query: &Query) -> Board {
    let events: Vec<&Event> = snap.scoped(query).collect();
    let mut board = Board {
        events: events.len() as u32,
        tokens: TokenSpend::Unknown,
        tools: tool_histogram(events.iter().copied()),
        ..Board::default()
    };

    let mut sessions: BTreeSet<SessionId> = BTreeSet::new();
    let mut nax_sessions: BTreeSet<SessionId> = BTreeSet::new();
    let mut by_thread: BTreeMap<ThreadId, Vec<&Event>> = BTreeMap::new();
    for e in &events {
        sessions.insert(e.thread.session().clone());
        by_thread.entry(e.thread.clone()).or_default().push(*e);
        if let EventBody::UserTurn { slash, .. } = &e.body {
            if slash.iter().any(|s| s == "nax-mode") {
                nax_sessions.insert(e.thread.session().clone());
            }
        }
    }
    board.sessions = sessions.len() as u32;

    let mut seen_edges: BTreeSet<(NodeKey, NodeKey)> = BTreeSet::new();
    for (thread, thread_events) in &by_thread {
        let nax = nax_sessions.contains(thread.session());
        let judged = judge(thread_events, catalog, nax);
        let mut playbook_reads = BTreeSet::new();
        let mut skill_reads = BTreeSet::new();
        for inv in &judged {
            match &inv.doc {
                DocRef::Playbook(n) => {
                    playbook_reads.insert(n.clone());
                }
                DocRef::Skill(n) => {
                    skill_reads.insert(n.clone());
                }
                _ => {}
            }
            let key = NodeKey::Doc(inv.doc.clone());
            let lane = inv.doc.lane();
            let node = board.nodes.entry(key.clone()).or_insert_with(|| Node {
                key: key.clone(),
                lane,
                label: inv.doc.label(),
                tally: Tally::blank(),
                ambient: 0,
            });
            node.tally.add_verdict(&inv.verdict);
            if inv.ambient {
                node.ambient += 1;
            }
        }
        for pb in &playbook_reads {
            let Some(expected) = catalog.playbook_mentions.get(pb) else {
                continue;
            };
            board.routing_expected += expected.len() as u32;
            board.routing_hits += expected.intersection(&skill_reads).count() as u32;
            for skill in expected.intersection(&skill_reads) {
                let from = NodeKey::Doc(DocRef::Playbook(pb.clone()));
                let to = NodeKey::Doc(DocRef::Skill(skill.clone()));
                if seen_edges.insert((from.clone(), to.clone())) {
                    board.edges.push(Edge { from, to });
                }
            }
        }
    }

    for e in &events {
        if let EventBody::Tool(ToolCall::Spawn { model, .. }) = &e.body {
            let key = NodeKey::Model(model.clone());
            let node = board.nodes.entry(key.clone()).or_insert_with(|| Node {
                key: key.clone(),
                lane: Lane::Models,
                label: model.label(),
                tally: Tally::blank(),
                ambient: 0,
            });
            node.tally.contacts += 1;
        }
    }

    if let Some(journey) = latest_journey(&events) {
        let turns = events
            .iter()
            .filter(|e| {
                matches!(e.body, EventBody::UserTurn { .. })
                    && matches!(e.thread, ThreadId::Root(_))
            })
            .count() as u32;
        board
            .nodes
            .entry(NodeKey::Prompt)
            .or_insert_with(|| Node {
                key: NodeKey::Prompt,
                lane: Lane::Prompt,
                label: theme::CHAT_LABEL.into(),
                tally: Tally::blank(),
                ambient: 0,
            })
            .tally
            .contacts = turns;
        for stop in &journey.stops {
            ensure_stop_node(&mut board, stop, catalog);
        }
        board.prompt_preview = journey.preview;
        board.live = journey.live;
        for pair in journey.stops.windows(2) {
            let from = node_key_for(&board, &pair[0], catalog);
            let to = node_key_for(&board, &pair[1], catalog);
            if seen_edges.insert((from.clone(), to.clone())) {
                board.edges.push(Edge { from, to });
            }
        }
        board.visits = journey.stops.len().saturating_sub(1) as u32;
        if !journey.stops.is_empty() {
            board.flights.push(Flight {
                stops: journey.stops,
            });
        }
    }

    board
}

fn ensure_stop_node(board: &mut Board, label: &str, catalog: &Catalog) {
    if board.nodes.values().any(|n| n.label == label) {
        return;
    }
    let key = node_key_for(board, label, catalog);
    let lane = match &key {
        NodeKey::Prompt => Lane::Prompt,
        NodeKey::Doc(doc) => doc.lane(),
        NodeKey::Model(_) => Lane::Models,
    };
    board.nodes.entry(key.clone()).or_insert_with(|| Node {
        key,
        lane,
        label: label.into(),
        tally: Tally::blank(),
        ambient: 0,
    });
}

fn node_key_for(board: &Board, label: &str, catalog: &Catalog) -> NodeKey {
    if label == theme::CHAT_LABEL {
        return NodeKey::Prompt;
    }
    if let Some(node) = board.nodes.values().find(|n| n.label == label) {
        return node.key.clone();
    }
    NodeKey::Doc(resolve_doc(label, catalog))
}

struct Journey {
    stops: Vec<String>,
    preview: Option<String>,
    live: bool,
}

fn latest_journey(events: &[&Event]) -> Option<Journey> {
    let start = latest_root_user_turn(events)?;
    let thread = &events[start].thread;
    let preview = match &events[start].body {
        EventBody::UserTurn { text, .. } => Some(prompt_preview(text)),
        _ => None,
    };
    let mut stops = vec![theme::CHAT_LABEL.to_string()];
    let mut live = true;
    for e in events.iter().skip(start + 1) {
        if e.thread != *thread {
            continue;
        }
        match &e.body {
            EventBody::UserTurn { .. } => break,
            EventBody::Tool(ToolCall::ReadDoc(doc)) => {
                let label = doc.label();
                if stops.last() != Some(&label) {
                    stops.push(label);
                }
            }
            EventBody::TurnEnded => {
                live = false;
                break;
            }
            _ => {}
        }
    }
    Some(Journey {
        stops,
        preview,
        live,
    })
}

fn latest_root_user_turn(events: &[&Event]) -> Option<usize> {
    let mut best_open = None;
    let mut best_any = None;
    for (i, e) in events.iter().enumerate() {
        if matches!(e.thread, ThreadId::Child(_, _)) {
            continue;
        }
        if !matches!(e.body, EventBody::UserTurn { .. }) {
            continue;
        }
        best_any = Some(match best_any {
            None => i,
            Some(j) => pick_later(events, i, j),
        });
        if turn_is_open(events, i) {
            best_open = Some(match best_open {
                None => i,
                Some(j) => pick_later(events, i, j),
            });
        }
    }
    best_open.or(best_any)
}

fn turn_is_open(events: &[&Event], start: usize) -> bool {
    let thread = &events[start].thread;
    for e in events.iter().skip(start + 1) {
        if e.thread != *thread {
            continue;
        }
        match &e.body {
            EventBody::UserTurn { .. } | EventBody::TurnEnded => return false,
            _ => {}
        }
    }
    true
}

fn pick_later(events: &[&Event], i: usize, j: usize) -> usize {
    match (&events[i].stamp, &events[j].stamp) {
        (Some(a), Some(b)) if a > b => i,
        (Some(a), Some(b)) if a < b => j,
        (Some(_), None) => i,
        (None, Some(_)) => j,
        _ => i,
    }
}

pub struct Directory {
    pub workspaces: Vec<WorkspaceEntry>,
}

pub struct WorkspaceEntry {
    pub id: String,
    pub chats: Vec<ChatEntry>,
}

pub struct ChatEntry {
    pub id: String,
    pub label: String,
}

pub fn directory(snap: &Snapshot) -> Directory {
    let mut map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for e in &snap.events {
        if matches!(e.thread, ThreadId::Child(_, _)) {
            continue;
        }
        let session = e.thread.session();
        let chats = map.entry(session.workspace.clone()).or_default();
        let label = chats.entry(session.conversation.clone()).or_default();
        if label.is_empty() {
            if let EventBody::UserTurn { text, .. } = &e.body {
                *label = prompt_preview(text);
            }
        }
    }
    let workspaces = map
        .into_iter()
        .map(|(id, chats)| WorkspaceEntry {
            id,
            chats: chats
                .into_iter()
                .map(|(chat_id, label)| ChatEntry {
                    label: if label.is_empty() {
                        short_id(&chat_id)
                    } else {
                        label
                    },
                    id: chat_id,
                })
                .collect(),
        })
        .collect();
    Directory { workspaces }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn prompt_preview(text: &str) -> String {
    let body = text.split("<user_query>").nth(1).unwrap_or(text);
    let body = body.split("</user_query>").next().unwrap_or(body).trim();
    let clipped: String = body.chars().take(96).collect();
    if body.chars().count() > 96 {
        format!("{clipped}…")
    } else {
        clipped
    }
}

struct Invocation {
    doc: DocRef,
    verdict: Verdict,
    ambient: bool,
}

fn judge(events: &[&Event], catalog: &Catalog, nax: bool) -> Vec<Invocation> {
    let settled = events
        .iter()
        .any(|e| matches!(e.body, EventBody::TurnEnded));
    let mut reads: BTreeMap<String, bool> = BTreeMap::new();
    let mut cites: BTreeMap<String, bool> = BTreeMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut skipped: BTreeSet<String> = BTreeSet::new();
    let mut acted_after_cite: BTreeSet<String> = BTreeSet::new();
    let mut leaf_followed: BTreeSet<String> = BTreeSet::new();
    let mut ambient_docs: BTreeSet<String> = BTreeSet::new();
    let mut pending_cites: BTreeSet<String> = BTreeSet::new();

    for e in events {
        match &e.body {
            EventBody::UserTurn { .. } => {}
            EventBody::AssistantText { text } => {
                for name in catalog
                    .skills
                    .iter()
                    .chain(catalog.principles.iter())
                    .chain(catalog.playbooks.iter())
                {
                    if cites_name(text, name) {
                        cites.entry(name.clone()).or_insert(false);
                        pending_cites.insert(name.clone());
                    }
                }
            }
            EventBody::Tool(ToolCall::ReadDoc(doc)) => {
                let key = doc_key(doc);
                reads.entry(key.clone()).or_insert(false);
                if !nax {
                    ambient_docs.insert(key.clone());
                }
                if let DocRef::Reference { skill, .. } = doc {
                    if reads.contains_key(skill) {
                        leaf_followed.insert(skill.clone());
                        used.insert(skill.clone());
                    }
                }
            }
            EventBody::Tool(ToolCall::Todo { items }) => {
                for item in items {
                    for name in catalog.playbooks.iter().chain(catalog.skills.iter()) {
                        if item.text.contains(name.as_str()) {
                            if item.skipped {
                                skipped.insert(name.clone());
                            } else {
                                used.insert(name.clone());
                            }
                        }
                    }
                }
            }
            EventBody::Tool(
                ToolCall::Edit { .. } | ToolCall::Shell { .. } | ToolCall::Spawn { .. },
            ) => {
                for cite in std::mem::take(&mut pending_cites) {
                    acted_after_cite.insert(cite.clone());
                    used.insert(cite);
                }
            }
            EventBody::Tool(_) | EventBody::TurnEnded => {}
        }
    }

    let mut out = Vec::new();
    let mut keys: BTreeSet<String> = BTreeSet::new();
    keys.extend(reads.keys().cloned());
    keys.extend(cites.keys().cloned());
    keys.extend(skipped.iter().cloned());

    for key in keys {
        let doc = resolve_doc(&key, catalog);
        let read = reads.contains_key(&key);
        let cited = cites.contains_key(&key);
        let ambient = ambient_docs.contains(&key);
        let verdict = if skipped.contains(&key) {
            Verdict::Miss(MissReason::StepSkipped)
        } else if cited && !read {
            Verdict::Miss(MissReason::CitedWithoutRead)
        } else if leaf_followed.contains(&key) {
            Verdict::Hit(HitEvidence::LeafFollowed)
        } else if used.contains(&key) && acted_after_cite.contains(&key) {
            Verdict::Hit(HitEvidence::CitedThenActed)
        } else if used.contains(&key) {
            Verdict::Hit(HitEvidence::StepAdopted)
        } else if read && !used.contains(&key) {
            if settled {
                Verdict::Miss(MissReason::ReadNeverUsed)
            } else {
                Verdict::Unknown
            }
        } else {
            Verdict::Unknown
        };
        out.push(Invocation {
            doc,
            verdict,
            ambient,
        });
    }
    out
}

fn doc_key(doc: &DocRef) -> String {
    match doc {
        DocRef::Skill(n) | DocRef::Principle(n) | DocRef::Playbook(n) => n.clone(),
        DocRef::Reference { skill, .. } => skill.clone(),
    }
}

fn resolve_doc(key: &str, catalog: &Catalog) -> DocRef {
    if catalog.playbooks.contains(key) {
        DocRef::Playbook(key.to_string())
    } else if catalog.principles.contains(key) {
        DocRef::Principle(key.to_string())
    } else {
        DocRef::Skill(key.to_string())
    }
}

fn cites_name(text: &str, name: &str) -> bool {
    text.contains(&format!("`{name}`"))
        || text.contains(&format!("**{name}**"))
        || text.contains(&format!("/{name}"))
}

pub fn inspect(board: &Board, label: &str) -> Option<Inspection> {
    let node = board.nodes.values().find(|n| n.label == label)?;
    let detail = if node.key == NodeKey::Prompt {
        board.prompt_preview.clone()
    } else {
        None
    };
    Some(Inspection {
        label: node.label.clone(),
        lane: node.lane.title().to_string(),
        hit: node.tally.hit,
        miss: node.tally.miss,
        unknown: node.tally.unknown,
        contacts: node.tally.contacts,
        ambient: node.ambient,
        detail,
    })
}

pub fn inspect_series(
    snap: &Snapshot,
    catalog: &Catalog,
    query: &Query,
    label: &str,
) -> InspectSeries {
    let events: Vec<&Event> = snap.scoped(query).collect();
    let mut nax_sessions: BTreeSet<SessionId> = BTreeSet::new();
    let mut by_thread: BTreeMap<ThreadId, Vec<&Event>> = BTreeMap::new();
    for e in &events {
        by_thread.entry(e.thread.clone()).or_default().push(*e);
        if let EventBody::UserTurn { slash, .. } = &e.body {
            if slash.iter().any(|s| s == "nax-mode") {
                nax_sessions.insert(e.thread.session().clone());
            }
        }
    }
    let mut series = InspectSeries::default();
    for (thread, thread_events) in &by_thread {
        if matches!(thread, ThreadId::Child(_, _)) {
            continue;
        }
        let nax = nax_sessions.contains(thread.session());
        if label == theme::CHAT_LABEL {
            let reads = thread_events
                .iter()
                .filter(|e| matches!(e.body, EventBody::Tool(ToolCall::ReadDoc(_))))
                .count() as u32;
            series.hits.push(reads);
            series.misses.push(0);
            continue;
        }
        let judged = judge(thread_events, catalog, nax);
        let mut hit = 0;
        let mut miss = 0;
        for inv in judged {
            if inv.doc.label() != label {
                continue;
            }
            match inv.verdict {
                Verdict::Hit(_) => hit += 1,
                Verdict::Miss(_) => miss += 1,
                Verdict::Unknown => {}
            }
        }
        let spawns = thread_events
            .iter()
            .filter(|e| match &e.body {
                EventBody::Tool(ToolCall::Spawn { model, .. }) => model.label() == label,
                _ => false,
            })
            .count() as u32;
        if hit + miss + spawns == 0 {
            series.hits.push(0);
            series.misses.push(0);
        } else {
            series.hits.push(hit.max(spawns));
            series.misses.push(miss);
        }
    }
    series
}

pub fn advise(board: &Board) -> Vec<Advice> {
    let mut rows: Vec<Advice> = board
        .nodes
        .values()
        .filter(|n| n.lane == Lane::Skills)
        .filter(|n| n.tally.contacts > 0)
        .map(|n| {
            let hit = n.tally.hit;
            let miss = n.tally.miss;
            let unknown = n.tally.unknown;
            let contacts = n.tally.contacts;
            let rate = hit as f32 / contacts.max(1) as f32;
            let tier = if rate >= 0.85 && contacts >= 3 {
                Tier::S
            } else if rate >= 0.7 {
                Tier::A
            } else if rate >= 0.45 {
                Tier::B
            } else if rate >= 0.25 {
                Tier::C
            } else if hit > 0 {
                Tier::D
            } else {
                Tier::F
            };
            let kind = if miss > 0 && hit == 0 && unknown == 0 {
                AdviceKind::Remove {
                    reason: "Read or cited and never used after settle.".into(),
                }
            } else if n.ambient > 0 && hit == 0 {
                AdviceKind::Remove {
                    reason: "Only opened as ambient context. Drop it from the default path.".into(),
                }
            } else if miss > 0 && hit > 0 {
                AdviceKind::Optimize {
                    reason: "Mixed HIT/MISS. Tighten when this skill is allowed to fire.".into(),
                }
            } else if unknown > hit && unknown > miss {
                AdviceKind::Optimize {
                    reason:
                        "Most calls are still open. Confirm it is required before the turn ends."
                            .into(),
                }
            } else {
                AdviceKind::Keep
            };
            Advice {
                label: n.label.clone(),
                lane: n.lane.title().to_string(),
                tier,
                hit,
                miss,
                unknown,
                contacts,
                kind,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(b.contacts.cmp(&a.contacts))
            .then(a.label.cmp(&b.label))
    });
    rows
}

use crate::domain::{Board, Lane, TokenSpend};
use serde::Serialize;

#[derive(Serialize)]
pub struct Report {
    pub sessions: u32,
    pub events: u32,
    pub tokens: String,
    pub routing: String,
    pub tools: Vec<(String, u32)>,
    pub nodes: Vec<ReportNode>,
}

#[derive(Serialize)]
pub struct ReportNode {
    pub lane: String,
    pub label: String,
    pub hit: u32,
    pub miss: u32,
    pub unknown: u32,
    pub contacts: u32,
    pub ambient: u32,
}

pub fn from_board(board: &Board) -> Report {
    let mut nodes: Vec<ReportNode> = board
        .nodes
        .values()
        .map(|n| ReportNode {
            lane: n.lane.title().into(),
            label: n.label.clone(),
            hit: n.tally.hit,
            miss: n.tally.miss,
            unknown: n.tally.unknown,
            contacts: n.tally.contacts,
            ambient: n.ambient,
        })
        .collect();
    nodes.sort_by(|a, b| b.contacts.cmp(&a.contacts));
    Report {
        sessions: board.sessions,
        events: board.events,
        tokens: match board.tokens {
            TokenSpend::Unknown => "unknown".into(),
            TokenSpend::Estimated { chars } => format!("estimated {chars} chars"),
            TokenSpend::Reported { input, output } => format!("reported {input}+{output}"),
        },
        routing: format!(
            "{}/{} playbook skills also read",
            board.routing_hits, board.routing_expected
        ),
        tools: board.tools.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        nodes,
    }
}

pub fn print_text(board: &Board) {
    println!(
        "sessions {}  events {}  tokens unknown  routing {}/{}",
        board.sessions, board.events, board.routing_hits, board.routing_expected
    );
    for lane in Lane::all() {
        let mut row: Vec<_> = board.nodes.values().filter(|n| n.lane == lane).collect();
        row.sort_by(|a, b| b.tally.contacts.cmp(&a.tally.contacts));
        if row.is_empty() {
            continue;
        }
        println!("{}", lane.title());
        for n in row {
            println!(
                "  {:<28} hit {:>3}  miss {:>3}  unk {:>3}  n {:>3}",
                n.label, n.tally.hit, n.tally.miss, n.tally.unknown, n.tally.contacts
            );
        }
    }
}

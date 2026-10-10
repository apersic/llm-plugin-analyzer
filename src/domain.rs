use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct SessionId {
    pub workspace: String,
    pub conversation: String,
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum ThreadId {
    Root(SessionId),
    Child(SessionId, String),
}

impl ThreadId {
    pub fn session(&self) -> &SessionId {
        match self {
            ThreadId::Root(s) | ThreadId::Child(s, _) => s,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct EventId {
    pub source: u64,
    pub line: u32,
    pub part: u16,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub id: EventId,
    pub thread: ThreadId,
    pub stamp: Option<String>,
    pub body: EventBody,
}

#[derive(Clone, Debug)]
pub enum EventBody {
    UserTurn { slash: Vec<String>, text: Arc<str> },
    AssistantText { text: Arc<str> },
    Tool(ToolCall),
    TurnEnded,
}

#[derive(Clone, Debug)]
pub enum ToolCall {
    ReadDoc(DocRef),
    Spawn {
        model: ModelRef,
        prompt: Arc<str>,
        description: Arc<str>,
    },
    Todo {
        items: Vec<TodoItem>,
    },
    Edit {
        path: PathBuf,
    },
    Shell {
        command_head: Arc<str>,
    },
    Other {
        name: String,
    },
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum DocRef {
    Skill(String),
    Principle(String),
    Playbook(String),
    Reference { skill: String, file: String },
}

impl DocRef {
    pub fn label(&self) -> String {
        match self {
            DocRef::Skill(n) => n.clone(),
            DocRef::Principle(n) => format!("principle-{n}"),
            DocRef::Playbook(n) => n.clone(),
            DocRef::Reference { skill, file } => format!("{skill}/{file}"),
        }
    }

    pub fn lane(&self) -> Lane {
        match self {
            DocRef::Skill(_) | DocRef::Reference { .. } => Lane::Skills,
            DocRef::Principle(_) => Lane::Principles,
            DocRef::Playbook(_) => Lane::Playbooks,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TodoItem {
    pub text: Arc<str>,
    pub skipped: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum ModelRef {
    Requested(String),
    Inherited,
    Unknown,
}

impl ModelRef {
    pub fn label(&self) -> String {
        match self {
            ModelRef::Requested(s) => s.clone(),
            ModelRef::Inherited => "inherit-parent".into(),
            ModelRef::Unknown => "unknown".into(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub enum TokenSpend {
    #[default]
    Unknown,
    Estimated {
        chars: u64,
    },
    Reported {
        input: u64,
        output: u64,
    },
}

#[derive(Clone, Debug)]
pub enum Verdict {
    Hit(HitEvidence),
    Miss(MissReason),
    Unknown,
}

#[derive(Clone, Debug)]
pub enum HitEvidence {
    StepAdopted,
    CitedThenActed,
    LeafFollowed,
}

#[derive(Clone, Debug)]
pub enum MissReason {
    ReadNeverUsed,
    CitedWithoutRead,
    StepSkipped,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum Lane {
    Prompt,
    Playbooks,
    Skills,
    Principles,
    Models,
}

impl Lane {
    pub fn title(self) -> &'static str {
        match self {
            Lane::Prompt => "Chat",
            Lane::Playbooks => "Playbooks",
            Lane::Skills => "Skills",
            Lane::Principles => "Principles",
            Lane::Models => "Models",
        }
    }

    pub fn all() -> [Lane; 5] {
        [
            Lane::Prompt,
            Lane::Playbooks,
            Lane::Skills,
            Lane::Principles,
            Lane::Models,
        ]
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum NodeKey {
    Prompt,
    Doc(DocRef),
    Model(ModelRef),
}

#[derive(Clone, Debug)]
pub struct Tally {
    pub hit: u32,
    pub miss: u32,
    pub unknown: u32,
    pub contacts: u32,
}

impl Tally {
    pub fn blank() -> Self {
        Self {
            hit: 0,
            miss: 0,
            unknown: 0,
            contacts: 0,
        }
    }

    pub fn add_verdict(&mut self, v: &Verdict) {
        self.contacts += 1;
        match v {
            Verdict::Hit(_) => self.hit += 1,
            Verdict::Miss(_) => self.miss += 1,
            Verdict::Unknown => self.unknown += 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub key: NodeKey,
    pub lane: Lane,
    pub label: String,
    pub tally: Tally,
    pub ambient: u32,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: NodeKey,
    pub to: NodeKey,
}

#[derive(Clone, Debug, Default)]
pub struct Board {
    pub nodes: BTreeMap<NodeKey, Node>,
    pub edges: Vec<Edge>,
    pub sessions: u32,
    pub events: u32,
    pub tokens: TokenSpend,
    pub tools: BTreeMap<String, u32>,
    pub routing_hits: u32,
    pub routing_expected: u32,
    pub flights: Vec<Flight>,
    pub prompt_preview: Option<String>,
    pub visits: u32,
    pub live: bool,
}

#[derive(Clone, Debug, Default)]
pub struct InspectSeries {
    pub hits: Vec<u32>,
    pub misses: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Tier {
    S,
    A,
    B,
    C,
    D,
    F,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::S => "S",
            Tier::A => "A",
            Tier::B => "B",
            Tier::C => "C",
            Tier::D => "D",
            Tier::F => "F",
        }
    }
}

#[derive(Clone, Debug)]
pub enum AdviceKind {
    Keep,
    Optimize { reason: String },
    Remove { reason: String },
}

#[derive(Clone, Debug)]
pub struct Advice {
    pub label: String,
    pub lane: String,
    pub tier: Tier,
    pub hit: u32,
    pub miss: u32,
    pub unknown: u32,
    pub contacts: u32,
    pub kind: AdviceKind,
}

#[derive(Clone, Debug)]
pub struct Flight {
    pub stops: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Inspection {
    pub label: String,
    pub lane: String,
    pub hit: u32,
    pub miss: u32,
    pub unknown: u32,
    pub contacts: u32,
    pub ambient: u32,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    All,
    Workspace(String),
    Chat {
        workspace: String,
        conversation: String,
    },
}

impl Scope {
    pub fn matches(&self, event: &Event) -> bool {
        let session = event.thread.session();
        match self {
            Scope::All => true,
            Scope::Workspace(workspace) => {
                session.workspace == *workspace || session.workspace.contains(workspace.as_str())
            }
            Scope::Chat {
                workspace,
                conversation,
            } => {
                session.conversation == *conversation
                    && (session.workspace == *workspace
                        || session.workspace.contains(workspace.as_str()))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Query {
    pub scope: Scope,
}

impl Query {
    pub fn all() -> Self {
        Self { scope: Scope::All }
    }
}

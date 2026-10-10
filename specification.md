# Specification

Facts for `llm-plugin-analyzer` 0.1.0. Commands, types, and labels match `src/` in this tree.

## Command

Binary name: `llm-plugin-analyzer`. Window title: `llm-plugin-analyzer`.

| Flag | Type | Default | Effect |
|---|---|---|---|
| `--plugin-root` | path | none | Load this plugin folder. |
| `--plugin` | string | none | Load the discovered plugin whose `id` or display name matches. |
| `--cursor-home` | path | `~/.cursor` | Root that holds `plugins/` and `projects/`. |
| `--workspace` | string | none | Fold `Scope::Workspace`. |
| `--chat` | string | none | Fold `Scope::Chat`. Uses `--workspace` when set, else an empty workspace string. |
| `--report` | bool | false | Print the board as text and exit. |
| `--json` | bool | false | Print the board as JSON and exit. |
| `--shot` | path | none | Write the graph PNG and exit. |
| `--width` | u32 | 1280 | PNG width for `--shot`. |
| `--height` | u32 | 800 | PNG height for `--shot`. |
| `--timings` | bool | false | Print scan, board, and layout timings on stderr. |

`--report`, `--json`, `--shot`, and `--timings` skip the window. The window size is 1280×800 and is not those PNG flags.

Plugin root resolution, in order:

1. `--plugin-root`
2. `--plugin` matched against discovered `id` or `name`
3. discovered plugin with `id` `guide-to-the-code`
4. first discovered plugin
5. `~/.cursor/plugins/local/guide-to-the-code`

`Scope` with no `--workspace` and no `--chat` is `All`.

## Plugin discovery

`catalog::discover` walks `cursor_home/plugins` to depth 6. A folder is a plugin root when it contains any `skills/**/SKILL.md`.

Id and display name come from `.cursor-plugin/plugin.json`, then `plugin.json`. Missing files are skipped. A folder whose name looks like a 32-character hex hash uses its parent name as the id.

If the same id appears twice, a path that contains a `local` segment wins.

`Catalog::load` requires a `skills/` directory. It records:

- skills: each `skills/<name>/SKILL.md` whose `<name>` does not start with `principle-`
- principles: each `skills/principle-<name>/SKILL.md`
- playbooks: each `*.md` under a `playbooks` directory
- playbook mentions: skill names that appear in that playbook text

`Catalog::classify` maps a filesystem path to `DocRef`.

## Transcripts

`ledger::Scanner` walks `cursor_home/projects/**/agent-transcripts/**/*.jsonl`. It caches by mtime and size. `Snapshot.digest` is stable for the same bytes plus catalog digest.

A path locates `SessionId { workspace, conversation }` and optional child thread id. Root threads are the conversation file. Child threads are nested agent files.

Parsed `EventBody` values:

- `UserTurn` with slash commands and text
- `AssistantText`
- `Tool` (`ReadDoc`, `Spawn`, `Todo`, `Edit`, `Shell`, `Other`)
- `TurnEnded` from a `turn_ended` line

Parent model and billed tokens are not in these files. `Board.tokens` is `TokenSpend::Unknown`.

The window polls every 250 ms. A refresh that sees the same disk stamp skips a rebuild.

## Scope

`Query.scope` is `All`, `Workspace(String)`, or `Chat { workspace, conversation }`.

`Workspace` matches when the session workspace equals the string or contains it. `Chat` also requires the conversation id.

`project::directory` lists workspaces and chats from root-thread events. A chat label is the first user-turn preview, else the first 8 characters of the id.

## Verdicts

A skill call is a `Read` of a classified plugin file. HIT/MISS is judged per root or child thread in `project::judge`.

HIT:

- `StepAdopted`: a `TodoWrite` item names the skill or playbook and is not `skip:`
- `CitedThenActed`: assistant text cites the name in `` `name` ``, `**name**`, or `/name`, then a later `Edit`, `Shell`, or `Spawn` runs in that thread
- `LeafFollowed`: a skill was Read, then a `references/` file under that skill was Read

MISS:

- `CitedWithoutRead`: citation, no Read
- `StepSkipped`: a `TodoWrite` item names it and is `skip:`
- `ReadNeverUsed`: the thread settled (`TurnEnded`) after a Read and never used it

UNKNOWN: the thread Read the file, has not used it, and has not settled.

Reads outside a `/nax-mode` session count as ambient. That rule is hardcoded to the `nax-mode` slash. Other plugins still load and graph. Their Reads are marked ambient until this check is generalized.

## Board

`project::board` folds the scoped snapshot into `Board`.

Nodes:

- `NodeKey::Prompt`, label `chat`
- `NodeKey::Doc(DocRef)` for skills, principles, playbooks, and references
- `NodeKey::Model(ModelRef)` for `Spawn` models

Edges are the latest journey's consecutive stops, plus playbook-to-skill pairs when the playbook text mentions a skill that the same thread also Read.

`Flight.stops` is the latest root-thread `Read` path, starting at `chat`. An open turn (no later `UserTurn` or `TurnEnded` on that thread) beats a settled one. `Board.live` is true when that journey has no `TurnEnded`. `Board.visits` is `stops.len() - 1`.

`InspectSeries` is one hit count and one miss count per root thread for the selected label. The `chat` node uses Read counts as hits and writes 0 misses.

## Advice

`project::advise` returns one `Advice` per Skills-lane node with `contacts > 0`.

Hit rate is `hit / contacts`.

| Tier | Rule |
|---|---|
| S | rate ≥ 0.85 and contacts ≥ 3 |
| A | rate ≥ 0.7 |
| B | rate ≥ 0.45 |
| C | rate ≥ 0.25 |
| D | hit > 0 and below C |
| F | hit == 0 |

Kind:

- `Remove` when miss > 0, hit == 0, unknown == 0
- `Remove` when ambient > 0 and hit == 0
- `Optimize` when miss > 0 and hit > 0
- `Optimize` when unknown is greater than both hit and miss
- `Keep` otherwise

Rows sort by tier, then contacts descending, then label.

## Window

Left HUD sections, in order: PLUGIN, SCOPE, STATS, PROMPT, GRAPH, LANES, TOOLS.

GRAPH controls:

- LIVE badge when `Board.live`
- `locked` checkbox, default off
- Nodes count
- Visits count

Camera:

- First valid central rect (width and height both > 80) uses `Camera::fitted`
- `locked` refits every frame
- Plugin or scope change sets `needs_fit` and refits once
- Unlocked drag pans. Scroll zooms.

Right inspect column: lane, optional prompt preview, Hit / Miss / Unknown pills, Contacts, Ambient, then **Hits** and **Misses** charts.

Skill roster is a second viewport. It lists `Advice` rows.

Layout is Fruchterman–Reingold with center gravity. A rebuild warm-starts from the previous positions.

## Headless report

`--json` fields: `sessions`, `events`, `tokens`, `routing`, `tools`, `nodes`. Each node has `lane`, `label`, `hit`, `miss`, `unknown`, `contacts`, `ambient`. Nodes sort by contacts descending.

`--report` prints `sessions`, `events`, `tokens unknown`, `routing hits/expected`, then one block per lane.

## Modules

| Module | Role |
|---|---|
| `catalog` | Discover plugins. Load skills, principles, playbooks. Classify paths. |
| `ledger` | Parse JSONL into `Event`. Cache by file stamp. |
| `domain` | Types for events, verdicts, board, scope, advice. |
| `project` | Fold a snapshot into `Board`, `Inspection`, `InspectSeries`, `Advice`. |
| `graph` | Layout, camera, PNG. |
| `report` | Text and JSON. |
| `theme` | Colors and type sizes. |
| `ui` | eframe window. |

## Limits

- No `tool_result` lines. Use is inferred.
- No billed tokens. `TokenSpend` variants `Estimated` and `Reported` exist and stay unused.
- Child-thread events are judged. The courier and inspect series use root threads.
- `--chat` without `--workspace` stores an empty workspace string. `Scope::Workspace` and `Scope::Chat` treat `contains("")` as a match, so that chat id hits in every workspace.
- `Catalog::load` fails if `skills/` is missing.

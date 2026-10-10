# llm-plugin-analyzer

See how a Cursor plugin fires on each prompt. The window is a live graph. Headless flags print a table, JSON, or a PNG.

Cursor does not log skill calls. This crate treats a `Read` of a plugin `SKILL.md` or playbook as the call, then judges HIT or MISS from later evidence in the same thread. A Read outside a `/nax-mode` session is counted as ambient. That slash name is still hardcoded.

## Run the window

Install a recent Rust toolchain. From this directory:

```text
cargo run --release
```

Debug `cargo run` folds transcripts on the UI thread. Use `--release` for the window.

The window discovers plugins under `~/.cursor/plugins`. Local copies win over cache copies of the same id. If you omit `--plugin`, it prefers `guide-to-the-code`, then the first discovered plugin.

To open on a named plugin or a folder:

```text
cargo run --release -- --plugin guide-to-the-code
cargo run --release -- --plugin-root C:\path\to\plugin
```

To fold one workspace or one chat:

```text
cargo run --release -- --workspace guide-to-the-code
cargo run --release -- --workspace guide-to-the-code --chat <conversation-id>
```

## Use the window

The left HUD has seven sections. Click a header to collapse it.

1. **PLUGIN.** Pick an installed plugin. **Skill roster** opens a second window with S–F tiers and keep, optimize, or remove notes for skills that have contacts.
2. **SCOPE.** Fold All, one workspace, or one chat. The chat list is the transcripts already on disk.
3. **STATS.** Sessions, events, links, and playbook-to-skill routing.
4. **PROMPT.** Preview of the latest root-thread user turn in this scope.
5. **GRAPH.** **LIVE** means that turn has not ended. **locked** keeps the camera fitted. Leave it off to pan and zoom. The window starts fitted and unlocked. **always show labels** paints every node name. Off, a label appears on hover, on the selected node, on `chat`, or when zoom is past 1.4.
6. **LANES.** Color key for Chat, Playbooks, Skills, Principles, and Models.
7. **TOOLS.** Tool-call counts in this scope.

The center is a force-directed graph. A `chat` node sits at the origin. A courier walks the latest prompt's `Read` path. Drag to pan. Scroll to zoom. Click a node to inspect it.

The right column shows Hit, Miss, Unknown, Contacts, and Ambient for the selected node, then two charts. **Hits** and **Misses** are one point per root thread.

## Print without a window

```text
cargo run --release -- --report
cargo run --release -- --json
cargo run --release -- --shot shots/graph.png
cargo run --release -- --timings
```

`--report` prints a text table and exits. `--json` prints the same board as JSON. `--shot` writes the graph canvas. `--width` and `--height` set the PNG size. `--timings` prints scan, board, and layout milliseconds on stderr.

`--cursor-home` overrides `~/.cursor` when your transcripts live elsewhere.

## Test

```text
cargo test
```

## Read next

[How to contribute](CONTRIBUTING.md) covers patches and reviews. [Code of conduct](CODE_OF_CONDUCT.md) is the floor for issues and PRs. Read [SECURITY.md](SECURITY.md) before you report a leak of transcripts or secrets.

[Specification](specification.md) lists flags, types, verdicts, and limits.

[Design](DESIGN.md) explains why a `Read` is a skill call and why HIT/MISS is judged later.

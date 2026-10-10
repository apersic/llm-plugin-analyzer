# Security policy

This crate reads Cursor JSONL transcripts under `~/.cursor/projects` and plugin files under `~/.cursor/plugins`. Those files can hold private chats, paths, and secrets. A bug that writes them to a screenshot, a report, or the network is in scope.

## Supported versions

`main` is the only supported line.

## How to report

Do not open a public issue for a vulnerability.

Open a [private security advisory](https://github.com/apersic/llm-plugin-analyzer/security/advisories/new) on this repository.

Include:

1. The flag, file, or HUD path. `--plugin-root`, `--shot`, `--json`, and `--cursor-home` are the usual ones.
2. What you ran. Redact tokens, keys, and transcript text you do not need.
3. What left the machine that should not have. A PNG, a JSON dump, or a window that showed another user's chat.

I will reply on the advisory. If I confirm it, I will patch `main` and credit you if you want that.

## Out of scope

- A wrong HIT or MISS. File a normal issue and add a case in `tests/verdicts.rs` if you can.
- Transcripts or keys you pasted into an issue yourself.
- A plugin skill that tells an agent to do something unsafe. That belongs with the plugin, not this viewer.

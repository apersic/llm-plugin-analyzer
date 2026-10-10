## What

What this PR changes. Name the file in `src/` or the doc.

## Why

The job this unblocks, or the bug it fixes. Link the issue if there is one.

## How I checked

```text
cargo test
```

If you changed a CLI flag or a HUD label:

```text
powershell -File scripts/check-docs.ps1
```

- [ ] `cargo test` passed
- [ ] I did not commit secrets, transcripts, `.audit/`, or `shots/`
- [ ] I updated `specification.md` when a flag, HUD label, or verdict rule changed
- [ ] I updated `DESIGN.md` when a why changed

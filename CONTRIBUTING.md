# How to contribute

This is a small Rust desktop crate. Patches and reviews are welcome. It is also a first public Rust app, so say when a type or a lifetime is wrong.

## Run the checks

```text
cargo test
cargo fmt
```

`cargo test` is 3 library tests plus the cases in `tests/verdicts.rs`. Add a case there when you change HIT, MISS, discovery, or live flight.

If you change a CLI flag or a HUD label, run `powershell -File scripts/check-docs.ps1`.

Do not commit `.audit/` or `shots/`. Both are gitignored working files.

## What to change

Keep the domain types in `src/domain.rs`. A new verdict, scope, or node kind belongs there first. Folding logic stays in `src/project.rs`. Parsing stays in `src/ledger.rs`. The window stays in `src/ui.rs`.

Do not add a second store next to the JSONL snapshot. Do not invent skill calls that are not a `Read` of a classified plugin file.

If you change a CLI flag, a HUD label, or a verdict rule, update [specification.md](specification.md) in the same patch. If you change a why, update [DESIGN.md](DESIGN.md).

## How to send work

Open a pull request against the default branch. Say what you ran. A review that only names style is still useful. A review that names a wrong verdict is better.

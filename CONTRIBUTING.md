# Contributing

Thanks for considering a contribution to `spice-lsp` / `spice-core`.

## Before you start

- For anything beyond a small fix (a new grammar feature, a new epic-sized
  chunk of work), open an issue first to discuss the approach — this
  project follows an epic-based workflow tracked in `progress.core.yaml`;
  large unplanned PRs are harder to review and merge.
- Bug fixes and doc improvements are always welcome without prior
  discussion.

## Development setup

```bash
git clone https://github.com/Elvis-codeur/spice-lsp
cd spice-lsp
pre-commit install   # installs the repo's pre-commit hooks
```

## Making a change

1. Create a branch off `master`.
2. Make your change. If you're touching `servers/core`, add or update tests
   in the same crate — see `AGENTS.md` for the project's testing
   conventions (unit tests live next to the code they test; real-file
   conformance tests live in `servers/core/tests/real_netlists.rs`).
3. Run the gate before opening a PR:

   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

4. If you added or changed a public item, make sure it has a `///` doc
   comment — `#![warn(missing_docs)]` is enabled crate-wide and CI treats
   `cargo doc` warnings as errors.
5. Open a PR describing what changed and why. CI (`.github/workflows/ci.yml`)
   runs `fmt`, `clippy`, `test`, and `doc` automatically.

## Reporting bugs

Please include a minimal netlist snippet that reproduces the issue,
which dialect (ngspice/Xyce) it's under, and what you expected vs. what
happened. If you found the bug against a real netlist file (not synthetic),
mention the source — real-world conformance testing is how most bugs in
this crate have been found so far.

## License

By contributing, you agree that your contributions are licensed under the
project's AGPL-3.0 license (see `LICENSE`).

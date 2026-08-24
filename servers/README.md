# servers/

- [`core/`](core/) — `general_spice_core`, the dialect-agnostic Rust parser and
  symbol-resolution library. See [`core/README.md`](core/README.md) for
  usage. This is where all the implemented functionality lives today.
- A `tower-lsp`-based binary wrapping `general_spice_core` for real LSP-over-stdio
  editor use (VS Code, Neovim, etc.) is planned but not started — see
  `docs/GRAMMAR.md` §9.2 and the root `README.md` for why `general_spice_core` is
  structured to make that a thin adapter rather than a rewrite.

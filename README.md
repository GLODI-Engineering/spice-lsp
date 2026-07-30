# spice-lsp

Language tooling for SPICE-family circuit netlists — ngspice, Xyce, and (planned) a reference tool.

## Goal

Provide an editor-agnostic Language Server Protocol (LSP) implementation for SPICE
netlists (diagnostics, hover, go-to-definition, completion for `.subckt`/`.model`
references, node names, and parameters), plus a VS Code extension that consumes it.

## Layout

- `docs/` — reference material: the unified netlist grammar (common vs.
  simulator-specific), design notes.
- `servers/` — the language server implementation(s). Language/runtime not yet
  decided (see `docs/`).
- `extension/` — the VS Code client extension (TypeScript), talks to the server(s)
  over stdio via the LSP client library.

## Status

Early stage: grammar research and design. No server or extension code yet.

## Simulators covered

| Simulator | Status |
|---|---|
| ngspice   | grammar research in progress |
| Xyce      | grammar research in progress |
| a reference tool   | not started — docs not yet available |

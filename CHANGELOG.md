# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

The crate has not been published since 0.1.0. What has landed since is recorded epic by epic in
`docs/progress.core.yaml`, which is the log this project actually keeps; this file is updated when a
version is published.

## [0.1.0] - 2026-07-31

Initial release of `general-spice-core` on crates.io.

### Added

- Dialect-aware lexer/preprocessor (comment stripping, line continuation)
  for ngspice and Xyce.
- Statement parser covering device instances, `.subckt`/`.model`/`.param`/
  `.func`, analysis statements, and dialect-specific keyword sets.
- Expression parsing: ngspice's separate compile-time and runtime
  (behavioral-source) grammars, and Xyce's unified grammar, including
  `TABLE()`/polynomial source forms.
- Symbol resolution: `.subckt`/`.model` name-uniqueness checking, subcircuit
  call resolution with circular-reference detection, model-reference
  resolution, and dialect-specific `.param` scope chains.
- Cross-dialect diagnostic lints for operators/functions whose meaning
  differs between ngspice and Xyce (`^`, `log()`).
- Multi-file `.include`/`.lib` resolution with virtual line-number mapping
  back to real file/line locations.
- Full rustdoc coverage of the public API (`#![warn(missing_docs)]` at zero
  warnings), with runnable doctested examples.
- Conformance-tested against real, independently-authored ngspice and Xyce
  netlists in addition to the unit test suite.

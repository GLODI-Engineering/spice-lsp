# Fixture provenance

These netlist files are used exclusively as parser-conformance test input
for `spice_core` (`servers/core/tests/real_netlists.rs`). They are not part
of the library's source code or its public API.

## `fixtures/ngspice/`

Copied unmodified from the ngspice project's own regression test suite
(`ngspice-46`, `tests/` directory — one or two small files sampled from
each of ~29 test categories: `misc`, `mesa`, `bsim3soi*`, `vbic`,
`transmission`, `polezero`, `parser`, `subckt-processing`, etc.). ngspice is
distributed under a BSD-style license (see the upstream project's
`COPYING` file). Filenames are prefixed with their original test category
directory, e.g. `polezero__pz2.cir`.

Upstream: <https://ngspice.sourceforge.io/>

## `fixtures/xyce/`

A mix of:
- Xyce's own `test/` directory example netlists (`CircuitPKG`,
  `XyceCInterface`, `SimulinkExamples`, etc.) — Xyce is distributed under
  GPL-3.0 (see the upstream project's `COPYING` file). Included here only
  as small, unmodified test-input data files, not as Xyce source code.
- Power-electronics converter benchmark netlists originally authored for
  this repository owner's own `Test-simulators-performance` project
  (buck/boost/flyback/LLC/DAB/three-phase-inverter Xyce netlists).

Upstream: <https://xyce.sandia.gov/>

## Why these are here

`spice_core` needs conformance testing against real, independently-authored
netlists — not just hand-written unit-test fixtures — to catch parsing gaps
that synthetic test cases miss. See `servers/core/tests/real_netlists.rs`
for how they're used.

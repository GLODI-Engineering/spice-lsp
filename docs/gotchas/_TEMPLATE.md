---
id: GOTCHA-NNN
discovered: YYYY-MM-DD
discovered_by: coder-agent          # coder-agent | tester-agent | master-agent | human
scope:                               # globs of paths this gotcha is relevant to
  - path/to/component/**
severity: medium                     # low | medium | high | critical
status: open                         # open | mitigated | resolved | wont-fix
reproducibility: full                # full | partial | unknown
tags: []
---

# GOTCHA-NNN — <short imperative title>

## Symptom

What the agent sees. Quote the exact error message, stack frame, log line,
or observed misbehavior. If the failure is silent (wrong output, no error),
say "silent" and describe the divergence.

## Reproduction

Minimum recipe to trigger the bug, including:

- **Environment**: OS, Rust/Node toolchain version, any non-default mount option.
- **Versions**: crate/package versions relevant to the bug.
- **Setup**: file layout, env vars, config flags.
- **Trigger**: the exact command to run.
- **Expected vs. actual**: what docs/GRAMMAR.md (or another spec) promises
  vs. what happens.

If the bug only reproduces on a specific host class, state it as a
**precondition** at the top of this section so future agents can skip the
gotcha when their environment differs.

## Root cause

The mechanism. "Why does this happen?", not "what happens to the user". If
unknown, write `unknown` and list the strongest leads.

## Fix / workaround

The smallest change that makes the bug go away. If multiple options exist,
list them with trade-offs.

## Prevention

What would have caught this earlier? A lint rule, a CI check, a doc line, a
test pattern. Be concrete enough that a future agent can implement it.

## References

- Upstream issue: <url> (accessed YYYY-MM-DD)
- Related commit: `<sha>`
- Datasheet/manual page / RFC: …

## History

- YYYY-MM-DD: Discovered while working on EPIC-XXX. (— discovered_by)
- YYYY-MM-DD: Workaround landed in commit `<sha>`.
- YYYY-MM-DD: Status changed to `mitigated`.

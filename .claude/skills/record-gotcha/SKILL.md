---
name: record-gotcha
description: Capture institutional memory for an off-epic bug — a problem the agent hit that is NOT covered by the current epic's acceptance criteria but will cost tokens to rediscover. Decides inline-only vs. long-form, writes the long-form document with reproduction conditions, appends the entry to the epic's `gotchas:` list, and triggers the index regenerator.
---

# `/record-gotcha` — Record an off-epic bug for future agents

Use this skill the moment you realize the bug you are chasing is **not in
`implementation.acceptance` or `tests.acceptance`** of your current epic.

Examples of the kind of thing this is for (illustrative, not from this
project yet):

- A SPICE manual's own PDF→markdown conversion silently drops a table row
  (e.g. a scale-factor suffix) because the source PDF used a non-standard
  glyph for a Greek mu — grep for the suffix comes up empty even though the
  manual documents it.
- `cargo test` passes locally but fails in CI because a golden-file fixture
  was saved with CRLF line endings on a Windows checkout.
- Xyce's `TABLE` syntax silently accepts a PSpice-style variant with no
  error, producing a wrong parse instead of a diagnostic — costs an hour to
  notice the parse was wrong at all.

Each costs an agent real time/tokens the first time. Recording it costs 30
seconds and saves the next agent the same cost.

## When NOT to use this skill

- The bug is in `implementation.acceptance` — that is the epic, fix it.
- The bug is a one-line typo in your own diff — fix it and move on.
- You can't reproduce the bug and can't describe a repro path — record it
  inline-only (Path A below), do not invoke long-form authoring.

---

## The gotcha-vs-epic-bug test

> Would fixing this bug satisfy **any** line of `implementation.acceptance`?
>
> - **Yes** → it is the epic. Don't record a gotcha. Fix it under
>   `/implement-epic` or `/fix-blocked`.
> - **No**  → it is a gotcha. Record it here.

---

## Two paths

### Path A — inline-only (fits in ≤ 5 lines)

Use when the gotcha can be fully described — symptom, cause, fix — in five
lines of prose, AND no reproduction recipe is needed because the cause is
self-evident from the description.

1. Pick an id of the form `GOTCHA-inline-YYYY-MM-DD[-slug]`.
2. Append to the current epic's `gotchas:` list in the progress file:

   ```yaml
   gotchas:
     - id: GOTCHA-inline-2026-08-01-crlf-fixture
       summary: |
         A golden-file fixture committed with CRLF line endings on a
         Windows checkout made cargo test fail only in CI. Fix: add
         `* text eol=lf` for fixture globs in .gitattributes.
   ```

3. Run `scripts/gotchas-index.sh` to refresh `docs/gotchas/INDEX.md`.
4. **Return to your previous skill.** This skill never owns the fix.

If your summary spills past 5 lines, **stop and switch to Path B** — that's
the rule, not a guideline.

### Path B — long-form document (default for anything non-trivial)

Use whenever:

- The summary does not fit in 5 lines, OR
- Reproduction requires specific environment / toolchain / data conditions, OR
- The root cause involves multiple components and would mislead the next
  reader if compressed, OR
- The workaround has trade-offs that another agent will need to weigh.

#### B.1 — Allocate the next id

```bash
ls docs/gotchas/ | grep -oE 'GOTCHA-[0-9]+' | sort -V | tail -1
```

Increment by 1. Pad to three digits: `GOTCHA-001`, `GOTCHA-002`, …

#### B.2 — Create the document

Copy `docs/gotchas/_TEMPLATE.md` to
`docs/gotchas/GOTCHA-NNN-<short-slug>.md` and fill **every** section:
front-matter (`id`, `discovered`, `discovered_by`, `scope`, `severity`,
`status`), Symptom, Reproduction, Root cause, Fix/workaround, Prevention,
References.

Keep the document under ~300 lines. If it exceeds that, you are probably
documenting a *project*, not a gotcha — split it.

#### B.3 — Append the epic reference

```yaml
gotchas:
  - id: GOTCHA-001
    summary: |
      <5-line summary — enough to decide whether to open the long-form doc>
      See docs/gotchas/GOTCHA-001-<slug>.md.
```

#### B.4 — Refresh the index

```bash
scripts/gotchas-index.sh
```

Regenerates `docs/gotchas/INDEX.md` from the front-matter of every
`GOTCHA-*.md`. Commit the index alongside your new doc.

#### B.5 — Return to your previous skill

`/record-gotcha` never owns the fix. After recording, resume
`/implement-epic`, `/fix-blocked`, or `/test-and-progress` from wherever you
paused.

---

## Front-matter contract (Path B)

The index script depends on this. Do not omit fields; use `unknown` or
`n/a` if you must.

```yaml
---
id: GOTCHA-001
discovered: 2026-08-01
discovered_by: tester-agent      # coder-agent | tester-agent | master-agent | human
scope:                            # globs the gotcha is relevant to
  - servers/core/**
severity: medium                  # low | medium | high | critical
status: open                      # open | mitigated | resolved | wont-fix
reproducibility: full             # full | partial | unknown
tags: []
---
```

---

## Hard limits

- **One gotcha = one document.** Don't bundle unrelated findings.
- **One gotcha = one owning epic.** The epic that discovered it. A later
  epic that *hits* the same gotcha cites the id in its `review:`/`blocked:`
  note — it does NOT add a duplicate entry.
- **Never silently fix-and-move-on.** If you spotted a gotcha and didn't
  record it, the next agent pays for it. Enforced in `/implement-epic`,
  `/fix-blocked`, and `/test-and-progress`.
- **Never edit** another epic's existing gotcha entries. Open a new one and
  link to the old one via `references:` if there's an update.

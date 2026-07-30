# Claude Code — Project Notes

The canonical agent guidance is in [`AGENTS.md`](./AGENTS.md). Read it first.
This file holds only Claude-Code-specific addenda.

## Claude-Code-specific addenda

- **Skills.** `.claude/skills/` — browse by name; each is a directory
  containing `SKILL.md`.
- **`/commit`.** Project-scoped skill in `.claude/skills/commit/`. The
  conventional-commits `commit-msg` hook enforces message format, so
  `/commit` is safe to use as-is.
- **Hooks.** Pre-commit hooks are installed at `.git/hooks/` (via
  `pre-commit install`). If a hook blocks you, do not pass `--no-verify`.
  Fix the underlying issue.

For everything else: [`AGENTS.md`](./AGENTS.md).

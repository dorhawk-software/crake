---
name: practices-docs
description: Keep a project's CLAUDE.md and engineering docs the dorhawk practices way - rules and one-line pointers in CLAUDE.md, detail in docs/engineering, every rule dated with who decided it, "until <date>" notes, milestones that say how each thing was proven, tried-by-real-people checklists, no handoff files that mix everything. Use when adding a rule or decision, when CLAUDE.md grows, when finishing a milestone, or when writing a handoff.
---

# CLAUDE.md and the engineering docs

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

Read `common/claude-md.md`, and `common/agent-working-rules.md` for how
agents work in these projects.

## Musts

- A new standing rule goes in CLAUDE.md in a line, with who decided it
  and the date. Its detail goes in `docs/engineering/<topic>.md`.
- Moving text out of CLAUDE.md moves it word for word.
- A finished milestone says what was built and how it was proven.
- Never write a handoff file that mixes rules, history and deferrals.

---
name: practices-commits
description: Commit, push and write the changelog the dorhawk practices way - the subject is the outcome, the body says why with numbers and how it was proven, Co-Authored-By yes and session IDs or links never, one commit identity per repository, push only when stated, a user-facing CHANGELOG line under Unreleased in the same commit, "not now" decisions written down in the same commit. Use before every commit, when writing a changelog line, or when deciding to defer something.
---

# Commits, changelog and deferrals

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

Read `common/commits.md`, `common/changelog.md` and `common/not-now.md`.

## Before each commit

1. Subject: the outcome, imperative ("Show the sender's address", not
   "Update reader.rs").
2. Body: why, with numbers where there are any, and how it was proven
   (the test, the measurement, the screen you looked at).
3. End with the Co-Authored-By line when an agent wrote the change.
   Never a session ID, session link or session UUID: the commit-msg hook
   refuses them.
4. A user-facing change: a CHANGELOG.md line under Unreleased, in the
   product's voice, for users, in the same commit.
5. Anything decided "not now", or "left as it is": a line in the
   project's not-now list, in the same commit.
6. Commit as the identity the project's CLAUDE.md names.
7. Push only when the project's rules or the person say so.

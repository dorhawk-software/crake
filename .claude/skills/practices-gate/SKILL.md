---
name: practices-gate
description: Run a project's practices lint gate (.practices/bin/lint) and coverage floors, and fix what they find the way the practices require. Use when the Claude Code hook reports "Lint gate failed", when a commit is refused by the pre-commit or commit-msg hook, when CI's lint or coverage step fails, or before handing work back in a project that has a .practices folder.
---

# The practices gate

A project with a `.practices/` folder treats every lint as an error. The
gate is the same everywhere: the Claude Code hook after cargo builds, the
pre-commit hook, and CI all run `.practices/bin/lint`.

## Run it

```sh
nice -n 19 .practices/bin/lint            # quiet when it passes
nice -n 19 .practices/bin/check-coverage  # about a minute: tests, measured
```

Run at the lowest CPU priority: the person is usually working on the same
machine.

## Fix what it finds: never silence it

| It says | Do |
| --- | --- |
| rustfmt failed | `cargo fmt`, then run the gate again. |
| clippy failed | Fix the code. No `#[allow]`. The one exception is a function over the length limit that the project is working down: `#[expect(clippy::too_many_lines, reason = "...")]`, which fails once it's no longer needed. Ask before adding one. |
| cargo deny failed | An advisory: update the crate (a security fix also gets a CHANGELOG line under Fixed, and the person should hear that a hotfix release is worth considering). A licence: don't widen deny.toml's list without asking; find another crate. |
| cargo machete failed | Remove the unused dependency. |
| private names | A client's name, host or address is in a file or message. Replace it with a made-up one (ACME, Globex, `*.example`, 192.0.2.x). Never quote the matched text back anywhere. |
| file sizes | Split the file by topic: a folder module with one file per part. A file in `.practices/size-allowlist` that shrank: lower its number there. |
| unsafe code | Move it behind a safe function in the crate the project allows it in (UNSAFE_ALLOWED in `.practices/config`). |
| git hooks | Run `.practices/bin/install-hooks` once in this clone. |
| coverage under a floor | Add tests for the lines it lists. Never lower a floor. When a crate is well over its floor, raise the floor in `.practices/coverage-floors`. |
| disk under its floor | Stop building. Tell the person what takes the room (the message lists it) and ask before going on. |
| session IDs in a commit message | Remove the trailer or link. A Co-Authored-By line is fine. |

## Before handing work back

- The gate passes, and the coverage floors hold when Rust files changed.
- If the person runs the app from the build folder, build it last
  (`cargo build`), so what they start is what you changed.
- Anything decided "not now" is written down in the project (its
  not-now list), in the same commit.

The practices behind each rule, with the incident that taught it:
https://github.com/dorhawk-software/practices (rust/lints.md,
rust/coverage.md, common/agent-working-rules.md).

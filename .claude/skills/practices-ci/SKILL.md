---
name: practices-ci
description: Change, speed up or debug CI the dorhawk practices way - CI minutes as a budget in private repositories, caches saved by main only, one build per job, time limits from measurements, third-party actions pinned by commit, secrets only where needed, the reusable rust-ci workflow. Use when editing .github/workflows, when CI is slow or fails, or when caches fill up.
---

# CI

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

Read `common/ci-caching.md`; for Rust also `rust/lints.md` ("Where the
gate runs") and the reusable workflows in `guides/how-it-works.md`.

## Musts

- A private repository pays for minutes: one Linux job unless the person
  agrees otherwise; the local hooks are the main gate.
- Quote measured timings (from the run's log), never estimates.
- Pin third-party actions by commit, with the version as a comment.
- Give secrets only to the steps that use them.

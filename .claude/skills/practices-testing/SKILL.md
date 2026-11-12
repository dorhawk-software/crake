---
name: practices-testing
description: Write, fix or review tests the dorhawk practices way - red before green, nothing touches the developer's machine (their mail, keyring, settings, network), one scenario per test run on every OS, failures that explain themselves, no retries for flaky tests, real-machine bugs become tests, coverage floors that only rise. Use when adding a feature or fixing a bug (the test comes first), when a test fails or flakes, or when coverage drops.
---

# Tests, the practices way

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

Read `common/testing.md`; for Rust also `rust/coverage.md`, and for
GPUI apps the journey kit in `rust/gpui.md`.

## Musts

- A bug fix starts with a test that fails for the bug. Then the fix.
- Tests never touch this machine's real data: no real home folder, mail,
  keyring, settings or network. Use temporary folders and fakes.
- A flaky test is a bug. Find the cause; never add retries or sleeps.
- A failure message says what was expected, what came, and where.
- A bug someone hit on a real machine becomes a test in the same commit.
- Coverage floors only rise: `.practices/coverage-floors`. Under a floor,
  add tests for the lines the check lists.
- Run heavy suites under `nice -n 19`.

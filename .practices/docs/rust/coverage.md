# Rust: coverage floors that only rise

Each crate's share of lines run by the tests stays at or above its floor. Raise a floor as the tests grow; never lower one to let a change through. Add the tests instead.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## A floor per crate, per OS

**What.** `.practices/coverage-floors` lists a floor in percent of lines for each crate, one for Linux and one for macOS:
```text
# crate      linux  macos
app          73     67
core         63     44
```

**Why.**
- A single number for the whole workspace hides a crate whose tests stopped ([15c892d](https://github.com/dorhawk-software/crake/commit/15c892d)).
- macOS needed floors of its own. Its system code runs in no test without root, so Linux's floors would have refused every commit made on a Mac ([eb13f5b](https://github.com/dorhawk-software/crake/commit/eb13f5b)).

**Apply.** Measure once with `.practices/bin/check-coverage`, then write today's numbers in as the floors.

**Check.** `.practices/bin/check-coverage` runs in the pre-commit hook when `.rs` files are staged, and in CI through `actions/rust-coverage`.

## Never lowered

**What.** CI compares the floors file with main's and fails if any floor went down.

**Why.** The rule held in Crake only as a sentence in CLAUDE.md. A check makes it hold without anyone remembering it.

**Apply.** `actions/rust-coverage` passes the base ref. Locally, `PRACTICES_BASE_REF=origin/main .practices/bin/check-coverage`.

**Check.** Open a pull request that lowers a floor; CI refuses it.

## Under the floor, say where

**What.** A crate under its floor prints the lines no test ran, per file.

**Why.** "73% < 74%" alone sends you hunting ([ac3bc91](https://github.com/dorhawk-software/crake/commit/ac3bc91)).

**Apply.** Built into `check-coverage`.

**Check.** Break a test on purpose and read the output.

## Test-harness code stays out of the share

**What.** Code that only exists to drive tests needing a real server is excluded from the share: scenario suites, end-to-end helpers, journey kits. Set it with `COVERAGE_EXCLUDE` (a regex) in `.practices/config`.

**Why.** The scenario suite's growth alone pulled Crake under its floor, though the app's own code hadn't changed. Without the harness, the app was at 73%, and the floor rose to that ([c6ebb10](https://github.com/dorhawk-software/crake/commit/c6ebb10)).

**Apply.** `COVERAGE_EXCLUDE='^app/src/(scenarios/|e2e_|journey_)'`.

**Check.** A review question when the regex grows: is this really harness code?

## One instrumented build runs everything

**What.** Coverage uses cargo-llvm-cov's `show-env` mode, which is plain `cargo test` with instrumentation. Extra test scripts given as arguments run in that same build.

**Why.** A separate build for the network tests cost about 11 minutes of a 15-minute job ([6c9fd90](https://github.com/dorhawk-software/crake/commit/6c9fd90)). Headless test binaries are built at opt-level 0 ([8bab495](https://github.com/dorhawk-software/crake/commit/8bab495)).

**Apply.** `.practices/bin/check-coverage scripts/netns-test.sh scripts/e2e-test.sh`.

**Check.** The CI log shows one compile of the workspace.

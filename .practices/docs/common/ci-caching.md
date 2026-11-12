# CI and caching on GitHub Actions

CI runs one build per job and cancels runs that a newer push replaced. It keeps its caches inside GitHub's 10 GB per repository on purpose: only main saves, keys carry the toolchain version, and replaced caches are pruned. Every lesson here cost Crake runs of 35 to 54 minutes before it was learned.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## The workflow header

**What.** Every CI workflow starts the same way. Concurrency cancels the run a new push replaced. Dependabot branches are tested only as pull requests. The token is read-only unless a job needs more.

**Why.** Only the newest commit matters, and CI minutes aren't free ([30bf3c7](https://github.com/dorhawk-software/crake/commit/30bf3c7)). Without `branches-ignore`, Dependabot branches ran twice, once as a push and once as a pull request ([525afd2](https://github.com/dorhawk-software/crake/commit/525afd2)).

**Apply.**
```yaml
on:
  push:
    branches-ignore: ["dependabot/**"]
  pull_request:
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
env:
  CARGO_TERM_COLOR: always
  CARGO_PROFILE_DEV_DEBUG: line-tables-only
```
Raise permissions per job only where needed: `actions: write` for pruning caches, `actions: read` to download artifacts.

**Check.** Review: does every workflow have this header? Does any job have more permissions than it uses?

## Private repos: CI minutes are a budget

**What.** In a private repository, every hosted runner minute is paid for. The local gate is the main gate, and CI only confirms it. Each push runs one Linux job. macOS and Windows run on hosted runners only for releases or when asked for.

**Why.** Prices are from [GitHub's Actions billing page](https://docs.github.com/en/billing/concepts/product-billing/github-actions), checked 2026-11-11.
- GitHub Free for organizations includes $12 a month of compute for private repos. It is counted in money, not minutes.
- That buys about 2,000 Linux 2-core minutes ($0.006/min), about 1,200 Windows minutes ($0.010/min), or about 193 macOS minutes ($0.062/min).
- One Crake-sized run on all three systems would spend the month in a few pushes.
- Standard runners are free for public repos.
- Self-hosted runners are free and don't use up the included amount. A $0.002/min charge for self-hosted runners, announced in December 2025 for March 2026, was postponed.

**Apply.**
- Install the hooks (`.practices/bin/install-hooks`) and the Claude hook. They run the same `.practices/bin` scripts that CI runs, so CI rarely finds anything new.
- Keep the workflow header's `cancel-in-progress: true`.
- Skip pushes that change only docs:
  ```yaml
  on:
    push:
      paths-ignore: ["**/*.md", "docs/**", "design/**"]
  ```
- Run hosted macOS and Windows jobs only on tags or `workflow_dispatch`: `if: startsWith(github.ref, 'refs/tags/') || github.event_name == 'workflow_dispatch'`.
- Run the heavy work on a self-hosted Linux runner, for example a spare Linux machine. Never use the machine you work on (see [testing.md](testing.md#nothing-touches-the-developers-machine)). The reusable workflows take a `runs-on` input, for example `runs-on: '["self-hosted", "linux"]'`.
- Caches stay main-only. The 10 GB limit applies to private repos too.

**Check.**
- Once a month, read the organization's billing page, or run `gh api /organizations/<org>/settings/billing/usage`.
- When telling anyone how long CI takes, quote measured run lengths: `gh run list --limit 10 --json createdAt,updatedAt,conclusion`.

## Caches are saved by main only

**What.** Every job restores caches, but only runs on main save them.

**Why.** GitHub keeps 10 GB per repository. It is a hard limit, the same for open source and private repos, with no paid tier. Branch copies evicted the caches the next run needed, and runs took 35 minutes with "No cache found" ([a31caa9](https://github.com/dorhawk-software/crake/commit/a31caa9)).

**Apply.** `actions/rust-setup` takes `save-cache: main` (the default) or `never`. With your own `actions/cache`, split it into `actions/cache/restore` everywhere and `actions/cache/save` with `if: github.ref == 'refs/heads/main'`.

**Check.** In Settings > Actions > Caches, every cache's ref is `refs/heads/main`.

## Spend the 10 GB on purpose

**What.**
- Full debug info is most of a target directory, so CI builds with line tables only.
- Jobs that build the same thing share one cache key.
- A job whose cache isn't worth its space restores but never saves.

**Why.** Even with main-only saves, Crake's caches came to 10.8 GB, and the Nix cache kept being evicted ([6bca175](https://github.com/dorhawk-software/crake/commit/6bca175)). The network job then reused the check job's dependencies through a shared key ([5536380](https://github.com/dorhawk-software/crake/commit/5536380)).

**Apply.**
- Set `CARGO_PROFILE_DEV_DEBUG: line-tables-only` in the workflow's `env`.
- Give `actions/rust-setup` the same `cache-shared-key` in jobs that build the same workspace.
- Use `save-cache: never` for jobs that only need to read.

**Check.** After a run on main, list the caches (`gh cache list`) and add up their size. Leave headroom under 10 GB.

## The toolchain version is in the cache key

**What.** A cache key names the exact compiler version.

**Why.** A Rust release mid-week left a cache full of builds the new compiler couldn't reuse. A key that already exists is never saved again, so every build that week ran from scratch ([4b6f5eb](https://github.com/dorhawk-software/crake/commit/4b6f5eb), [1544a21](https://github.com/dorhawk-software/crake/commit/1544a21)).

**Apply.** Pin the toolchain in `rust-toolchain.toml` (see [rust/lints.md](../rust/lints.md)), and the key follows it. `Swatinem/rust-cache` already includes the rustc version. Hand-rolled keys need it added: `<kind>-<rustc version>-<hash of Cargo.lock>`.

**Check.** A hand-written cache key contains the compiler version.

## Prune the caches a newer one replaced

**What.** After main's run saves its caches, a job deletes older caches of the same kind.

**Why.** After a Cargo.lock change, the old keys stay and count against 10 GB. In Crake they pushed out the packages cache, and one .deb build took 17 minutes instead of 7 ([6e8149c](https://github.com/dorhawk-software/crake/commit/6e8149c)).

**Apply.**
- Add a last job using `actions/prune-caches`, with `permissions: actions: write`. Its `needs:` lists every job that saves a cache, and it runs `if: always() && github.ref == 'refs/heads/main'`.
- A key's kind is the key minus its trailing hex groups, so a kind must not end in something that looks like hex. Write versions with dashes, not dots.
- `.practices/bin/prune-caches --dry-run` shows what would go.

**Check.** The prune job's `needs` matches the jobs that save. Crake once missed one, so its prune could run before that save.

## Sweep target directories you cache by hand

**What.** Before saving a hand-made cache of `target/`, delete every build unit the last build didn't use.

**Why.** Crake's packages cache grew from 2.6 to 7.2 GB and evicted two others ([c616c87](https://github.com/dorhawk-software/crake/commit/c616c87)).

**Apply.** Build with `--message-format=json`, then run `.practices/bin/sweep-target` on that output. `Swatinem/rust-cache` already cleans, so this is only for `actions/cache`.

**Check.** The cache size stays flat from week to week.

## One build per job

**What.** A job compiles the workspace once and runs everything from that build.

**Why.**
- `cargo test -p x` unifies features differently from `--workspace` and recompiles. One Windows job went from 6 to 22 minutes ([9668854](https://github.com/dorhawk-software/crake/commit/9668854)).
- A separate job for the network tests spent 11 of its 15 minutes rebuilding what the check job had just built ([6c9fd90](https://github.com/dorhawk-software/crake/commit/6c9fd90)).
- A macOS package built in release mode took 16 minutes, against 3 from the dev build ([7136554](https://github.com/dorhawk-software/crake/commit/7136554)).

**Apply.**
- Build with `--workspace`. Find test binaries in the JSON messages instead of rebuilding with `-p`.
- Run extra test scripts inside the coverage build (`.practices/bin/check-coverage scripts/x.sh`).
- Build headless test binaries with `CARGO_PROFILE_DEV_OPT_LEVEL=0` ([8bab495](https://github.com/dorhawk-software/crake/commit/8bab495)).

**Check.** In a job's log, "Compiling" appears for the workspace once.

## Split the slowest suite, not the whole run

**What.** When one system sets the run's length, split its slow suite across a matrix with `fail-fast: false`.

**Why.** Windows' gateway tests made the run 29.5 minutes instead of 23 ([b598a75](https://github.com/dorhawk-software/crake/commit/b598a75)).

**Apply.** Add a matrix of halves, each with a test filter. Measure first: `gh run view <id> --json jobs`.

**Check.** The longest job is less than 1.3 times the second longest.

## Time limits come from measurements

**What.** Steps that fetch from the network get a `timeout-minutes` set from durations you have measured.

**Why.** An Ubuntu mirror hung for 52 minutes, and the default job limit is 6 hours ([63fb60e](https://github.com/dorhawk-software/crake/commit/63fb60e)). A 5-minute limit then cut off a real 7-minute qemu install, so that step got 15 ([ffd6cfa](https://github.com/dorhawk-software/crake/commit/ffd6cfa)).

**Apply.** Use `timeout-minutes: 5` on apt steps. Composite actions can't set step timeouts, so `actions/rust-setup` wraps apt in `timeout 300`.

**Check.** Every network fetch has a limit, and a comment says what it was measured at.

## Artifacts are named per attempt

**What.** An artifact one job writes for another carries `${{ github.run_attempt }}` in its name.

**Why.** Artifacts survive re-runs. Re-running only the failed jobs found the previous attempt's "ready" file and failed with "bad handshake" ([e69d430](https://github.com/dorhawk-software/crake/commit/e69d430)).

**Apply.** `name: ready-${{ github.run_attempt }}`.

**Check.** Re-run one failed job and see that it waits for this attempt's artifact.

## Secrets reach only the steps that need them

**What.** A job tests whether a secret exists through `env` (`HAVE_X: ${{ secrets.X != '' }}`). Steps run `if: env.HAVE_X == 'true'`. The secret itself goes only to that step, never to a build.

**Why.** Forks and Dependabot runs have no secrets and must skip, not fail. Crates' build scripts run during builds, so a secret in a build's environment is readable by every dependency.

**Apply.** Use the pattern above. A skipped step says so in its log.

**Check.** No `secrets.` in a job-level `env` that a `cargo` step inherits.

## Pin third-party actions by commit

**What.** Shared actions refer to third-party actions by full commit SHA, with the version as a comment. Dependabot keeps them current.

**Why.** A tag can be moved to other code. This repo's actions run in every project that uses them. Crake pinned only one action by SHA.

**Apply.** `uses: Swatinem/rust-cache@<40-char sha> # v2.8.1`. Add `github-actions` to `dependabot.yml`.

**Check.** `grep -E 'uses: [^.].*@v[0-9]' actions .github` finds nothing in this repo.

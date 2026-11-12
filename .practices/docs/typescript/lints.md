# TypeScript: lints are errors

The gate for a TypeScript project: the compiler and ESLint, errors only,
visiting every file, with a size limit and a guard for names that clash
by case. It runs locally (the git hook and the Claude Code hook) and in
CI, from the same scripts.

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka).

## The gate: tsc and ESLint, errors only

**What.** `tsc --noEmit` and ESLint both pass with nothing reported. There
are no warnings: a rule is an error, or it's off with a counted reason
([conventions.md](conventions.md#every-disabled-lint-rule-has-a-counted-reason)).

**Why.** Warnings are noise. Obálka had 211 that nobody acted on before its
2026-09-09 audit. With errors only, its gate reports zero on 568 files in
about 11 s (tsc 3 s, ESLint 8 s, measured 2026-11-12 on a laptop).

**Apply.**
1. New projects: ESLint 9 with a flat config (`eslint.config.js`). The
   legacy `.eslintrc` format is end of life.
2. Turn on `eslint-comments/require-description`, so every disable says
   why.
3. Add the type-aware rules you rely on, for example
   `@typescript-eslint/no-deprecated` (Obálka found a deprecated option
   by hand before adding it).

**Check.** `npx tsc --noEmit && npx eslint . --max-warnings 0` passes.

## How `.practices/bin/lint` runs it

**What.** In a project with a `tsconfig.json`, the kit's gate runs
`tsc --noEmit`. Then it runs each npm script named in `NPM_CHECKS` in
`.practices/config` (default `lint`). The pre-commit hook runs the gate
when JavaScript or TypeScript files are staged, and also the npm script in
`NPM_TEST` (default `test`).

**Why.** Before adopting the kit, Obálka had no local gate at all. CI was
its only automatic check, so private names and secrets were caught after
the push, in a public repository. Its development had been on Linux, where
the iOS side couldn't be built.

**Apply.** In `.practices/config`:
```sh
NPM_CHECKS='lint changelog:check attributions:check'
NPM_TEST='test -- --silent'
```
Keep the whole local gate under about a minute. Slower checks (a store
build) run before a release instead
([mobile/ci.md](../mobile/ci.md#the-local-gate-builds-every-platform-before-a-tag)).

**Check.** Stage a file with a lint error: the commit is refused with the
finding.

## The gate visits every file

**What.** The linter's file globs cover every source the project runs:
`.ts`, `.tsx`, `.js`, `.mjs`, `.cjs`, and the dev scripts. A config that
can't parse a file must fail, not skip it.

**Why.** All five of Obálka's `.mjs` scripts were parse errors that ESLint
never visited. The gate reported zero while the release, audit and fixture
scripts went unchecked.

**Apply.** Give each extension an override in the config. Once, plant a
deliberate error in each kind of file and see the gate fail
([common/gates.md](../common/gates.md#prove-the-gate-can-fail)).

**Check.** `npx eslint --debug . 2>&1 | grep -c 'Linting'` matches the
number of source files.

## No two names that differ only by case

**What.** No two files in one folder have names that differ only in case,
and every import matches the file's case exactly.

**Why.** `typography.ts` sat next to `Typography.tsx`. Linux CI was green;
the first build on a Mac, whose disk ignores case, reported 196 type
errors ([3cbb48f](https://github.com/dorhawk-software/obalka/commit/3cbb48f)).

**Apply.** `forceConsistentCasingInFileNames: true` in tsconfig, plus a
test that lists the tracked files and fails on a case clash.

**Check.** `git ls-files | tr A-Z a-z | sort | uniq -d` prints nothing.

## Size limits

**What.** A source file has at most 700 lines. Generated files are left
out. A project adopting the limit lists today's bigger files in
`.practices/size-allowlist`, which only shrinks.

**Why.** Obálka had 16 files over 700 lines, the largest a 2,200-line
screen and a 2,400-line string table. The same limit holds in Rust
([rust/lints.md](../rust/lints.md#size-limits)).

**Apply.** `.practices/bin/check-size` checks `.ts` and `.tsx` files too.
Seed the allowlist with the measured sizes, and list each file in the
not-now list as work to split.

**Check.** `.practices/bin/check-size` passes.

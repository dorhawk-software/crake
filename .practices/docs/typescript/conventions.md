# TypeScript: code conventions

Braces everywhere, strict types, errors as typed codes the UI turns into
words, no stray console calls, platform code behind interfaces, and every
patched or disabled thing carrying its reason. Written for TypeScript
apps, React Native first.

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka),
a React Native app. Most of its history before 2026-11-10 was squashed
into one commit, so some reasons cite its specs (`specs/NNN-*`) instead.

## Braces around every body

**What.** Every `if`, `else`, `for`, `while` and `do` body has braces, even
a single statement.

**Why.** It was the owner's stated style from the start: a one-line body
invites an edit that adds a second line outside it, and braced bodies keep
diffs to the changed line. Obálka's constitution (v1.1.0) made it a rule.

**Apply.** ESLint: `curly: ['error', 'all']`. Never `if (x) return;`:
```ts
if (x) {
  return;
}
```

**Check.** `npx eslint .` passes with the rule on.

## Strict types, and every escape counted

**What.** `strict: true` at least. New projects also turn on
`noUncheckedIndexedAccess` and `forceConsistentCasingInFileNames`. An
`any` or `@ts-expect-error` carries a comment with its reason and what
removes it.

**Why.** Obálka inherited only `strict` from React Native's base config,
which sets `forceConsistentCasingInFileNames: false`. A file pair that
differed only by case passed on Linux and failed with 196 errors on a Mac
([3cbb48f](https://github.com/dorhawk-software/obalka/commit/3cbb48f)).
Its UI library's types needed an `any` wrapper, kept with a dated TODO.

**Apply.**
1. Extend the framework's base config, then turn on the flags above.
2. Don't add the `dom` lib to a React Native project. It type-checks
   globals the phone's engine (Hermes) doesn't have.
3. Each `any` gets `// any: <why>; remove when <condition>`.

**Check.** `grep -rn ': any\|as any\|@ts-' src` lists only lines with a
reason.

## Errors are typed codes; the UI picks the words

**What.** Services return a typed outcome: a code from a union, whether
it's recoverable, and a stable message key. The UI turns the key into
localized text. Every flow that can fail offers a way to retry.

**Why.** Obálka maps the server's error codes to its own keys; an unknown
code falls back to the server's own text. Its constitution requires an
error and a retry path in every flow, after a flow failed at its last step
with nothing to do but start over. The Rust practice is the same idea
([rust/conventions.md](../rust/conventions.md#errors-are-sentences)).

**Apply.**
```ts
type LoginError =
  | { code: 'invalidCredentials'; recoverable: true; messageKey: 'login.invalid' }
  | { code: 'serviceDown'; recoverable: true; messageKey: 'login.down' };
```
Never build user-facing text in a service. Never show a raw error message.

**Check.** No string shown to the user is built outside the i18n layer;
every error screen has a retry or a way back.

## No direct console calls in the app

**What.** Code under `src/` never calls `console.*` directly. It logs
through one logger that strips credentials and personal data.

**Why.** A phone's console reaches device logs and crash tooling. Obálka
forbids direct calls with a test (`noPlaintextSecrets`). Its logger then
went unused, which the audit flagged: a rule with an unused tool behind it
is half a rule.

**Apply.** A logger module with a deny-list of credential shapes. An ESLint
`no-console` error (or a source-scan test) in `src/`. Use the logger.

**Check.** `grep -rn 'console\.' src` is empty outside the logger.

## Heavy modules load lazily, with `require`

**What.** A heavy library (a PDF parser, a crypto fallback) is loaded
inside the function that needs it, with `require`, never `await import()`
under Metro.

**Why.** Metro turns `import()` into an async bundle. In Obálka it failed
with "Could not load bundle" and the caller took that for a valid empty
result. Loading at start-up slowed every launch.

**Apply.** `const { parse } = require('heavy-lib');` inside the function.
Long work yields between steps and has a cap, so the UI thread never
freezes.

**Check.** `grep -rn 'await import(' src` is empty.

## Every disabled lint rule has a counted reason

**What.** A rule is an error or it's off. Off means a comment with the
count of hits and why the rule fights a decision. A disable comment in
code says why.

**Why.** Obálka's audit of 2026-09-09 found 211 warnings nobody acted on,
hiding ten stale disables. It now runs errors only: three rules are off,
each with a count (inline styles 91, `void` 58, bitwise 41). See
[lints.md](lints.md).

**Apply.** In the ESLint config:
```js
// Off: 91 inline styles; the design system sets them from tokens (spec 028).
'react-native/no-inline-styles': 'off',
```
Turn on `eslint-comments/require-description`.

**Check.** No rule is set to `warn`. Every `eslint-disable` has a `--`
description.

## Platform code behind an interface, with a test double

**What.** Code that touches the OS (secure storage, notifications, the
camera) sits behind a small interface. The app gets the real one; tests
get an in-memory one.

**Why.** Obálka's `VaultKeyStorage` and `Notifier` let the core run in
Jest with no phone. Six service files still import from features or app,
so boundaries need a lint, not only a habit. It's the TypeScript form of
[OS code behind small modules](../rust/conventions.md#os-code-behind-small-modules).

**Apply.**
1. One interface per OS capability, in the services layer.
2. A platform module per OS when they differ (`.ios.ts`, `.android.ts`).
3. `no-restricted-imports` keeps services from importing UI.

**Check.** The services folder has no import from screens or app code.

## Patched dependencies: why, when to remove, pinned exactly

**What.** Every `patch-package` patch has a line in the docs: what it
fixes, the upstream issue, and the version that makes it unneeded. The
patched package is pinned to an exact version. A failed patch fails the
install.

**Why.** In Obálka, `npm audit fix` moved a patched package to a version
the patch no longer applied to, and the app lost per-account cookie
isolation until it was pinned
([83b239d](https://github.com/dorhawk-software/obalka/commit/83b239d)).
Four of its five patches had no written reason or removal condition.

**Apply.**
1. `"postinstall": "patch-package --error-on-fail"`.
2. Pin: `"react-native-blob-util": "0.24.9"`, no caret.
3. A table in `docs/engineering/` per patch: package, why, upstream link,
   remove when.

**Check.** Every file in `patches/` has a row; every patched package has
an exact version in package.json.

# TypeScript: testing with Jest

How a React Native app's tests stay fast and never touch the phone, the
network or real accounts: native modules faked as small in-memory stores,
fakes that refuse calls nobody set up, and a config that only sees the
project's own tests. The language-free rules are in
[common/testing.md](../common/testing.md).

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka).
Its suite: 3,078 tests in 282 suites, 13 s (measured 2026-11-12).

## Native modules faked as in-memory stores

**What.** Each native module the app uses has a manual mock in
`__mocks__/` that behaves like a tiny version of it: secure storage as a
map, the file system in memory, a cookie jar per URL. Mocks expose
`__seed` and `__reset` for tests.

**Why.** Jest can't load native code, and a bare `jest.fn()` returns
`undefined` where the app expects data. Obálka's cookie mock is a real jar,
so a test can prove one account's session never reaches another's.

**Apply.**
1. One file per module in `__mocks__/`, matching the package name.
2. Mock the SDKs that open sockets or install handlers (crash reporting)
   in `jest.setup.js`, as `jest.fn`s a test can assert on.
3. Reset every store in `afterEach`.

**Check.** A test can seed a store, run the code, and read back the
result, without any device.

## Fakes refuse what they weren't told

**What.** A fake service throws on any call the test didn't configure,
naming the call. An HTTP fake throws when it has no queued response for
the URL.

**Why.** In Obálka every server call is a legal act (signing in, marking
mail delivered), so an unexpected call must fail loudly. Its
`FakeTransport` throws "FakeTransport.X was called but not configured".
See [common/testing.md](../common/testing.md#fakes-refuse-what-they-werent-told).

**Apply.** Build fakes from a map of configured handlers; the default
handler throws with the method's name.

**Check.** Delete one configured handler in a test: it fails, naming it.

## No real network in tests

**What.** `jest.setup.js` replaces `fetch` (and any HTTP client's adapter)
with one that throws, naming the URL. Tests that need a response use a
fake.

**Why.** Obálka's isolation rested on dependency injection alone. Node's
real `fetch` was still reachable, so one forgotten fake could call a real
server.

**Apply.**
```js
global.fetch = (url) => {
  throw new Error(`no network in tests: ${url}`);
};
```

**Check.** A test that calls `fetch` unfaked fails with the URL.

## Jest sees only the project's tests

**What.** `testPathIgnorePatterns` and `modulePathIgnorePatterns` leave out
agent worktrees (`.claude/`), build output and native folders.
`moduleFileExtensions` lists `json` last.

**Why.** Agent worktrees inside the repository ran every suite once per
copy and caused module-name collisions. On a Mac, `../App` resolved to
`app.json` before `App.tsx` until json moved to the end
([3cbb48f](https://github.com/dorhawk-software/obalka/commit/3cbb48f)).

**Apply.** Spread the preset's `transform` and `haste` settings instead of
retyping them, so they can't go stale.

**Check.** `npx jest --listTests | grep -c worktrees` prints 0.

## Dev scripts are tested too

**What.** The scripts that release, check the changelog or audit
dependencies have Jest tests like app code.

**Why.** Obálka's release script folded the changelog and only then broke
Google Play's 500-character limit, after the tag was public
([815c2b3](https://github.com/dorhawk-software/obalka/commit/815c2b3)). A
release script is code that runs rarely, so its bugs show at the worst
time.

**Apply.** Export the scripts' logic as functions; test them on fixture
files.

**Check.** Each script under `scripts/` has a test file.

## A timeout is not a fix for slowness

**What.** Keep Jest's default timeout. A test that needs more names its
reason in the test. A suite that leaks handles ("a worker process has
failed to exit gracefully") is a bug to find.

**Why.** Obálka raised the global timeout to 30 s when the suite went red
while a build competed for the processor. That hid the slow test (a key
derivation bound to the event loop) instead of naming it.

**Apply.** Use cheap parameters for slow functions in tests. Run
`--detectOpenHandles` when the warning appears, and fix the leak.

**Check.** No global `testTimeout` above the default; no open-handle
warning.

## Coverage: measure, then set floors

**What.** Measure coverage before setting any floor. Then floors per area
that only rise, as in Rust ([rust/coverage.md](../rust/coverage.md)).

**Why.** Obálka had 3,078 tests and no coverage measurement at all, so
nobody knew which code no test ran.

**Apply.** `jest --coverage` once, read the share per folder, set each
folder's floor a little under it in `coverageThreshold`.

**Check.** CI runs with coverage and fails under a floor.

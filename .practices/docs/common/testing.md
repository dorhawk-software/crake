# Testing

Tests prove behaviour on every system the project supports, and they never touch the developer's computer. One suite stays green as the regression suite. Each new behaviour gets a test that failed before the change. A test that can't run somewhere says so; it never passes quietly.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake), where these rules were learned.

## Nothing touches the developer's machine

**What.** Development builds are never installed on the developer's computer. Tests that need privileges, networks or system services run sealed off. That means a container with no network, a read-only view of the host, a private `/run`, or a VM where a container would change the host kernel.

**Why.** The developer works on the same machine while builds and tests run. In Crake, root tests reached the host's real system bus through a chroot, and they talked to the developer's own VPN and DNS services. The fix gave each container its own `/run` ([13b2d72](https://github.com/dorhawk-software/crake/commit/13b2d72)). The rule itself was set on the first day ([0b63fbb](https://github.com/dorhawk-software/crake/commit/0b63fbb)).

**Apply.**
- Run binaries from the build folder, never from a system install.
- Wrap privileged tests in one script, for example `docker run --network none`, the host mounted read-only, `--tmpfs /run`, and no coverage output (`LLVM_PROFILE_FILE=/dev/null`).
- Use a VM for anything that loads kernel policy, such as AppArmor or SELinux.
- A test script checks its preconditions and refuses with the fix when one is missing. Some CI runners came with their own `resolv.conf`, and every scenario then failed with no reason given ([177f8c1](https://github.com/dorhawk-software/crake/commit/177f8c1)).

**Check.** Review question for every new test script: what on this machine could it read or change? The answer must be "nothing".

## A journey suite is the regression suite

**What.** One suite drives the real app as a user would, end to end, against real back ends on the toolkit's headless test platform. It stays green. Every change runs it, refactors above all. Every new entry point of the app gets a step in it (asked for 2026-10-11).

**Why.** Unit tests prove parts; the journey proves the app still works as a whole. Crake's journey runs in every Linux distribution's package test, on Windows and on macOS ([8cab7b5](https://github.com/dorhawk-software/crake/commit/8cab7b5)). It found Windows bugs that unit tests missed. Before it ran on a Mac, most real bugs had come from a Mac.

**Apply.**
- Make `main()` a thin call to `start(cx, Setup)`. `Setup` makes everything that touches the machine a choice: single instance, tray, file watchers, IPC clients, demo data ([7a28ba2](https://github.com/dorhawk-software/crake/commit/7a28ba2)).
- The test builds a `Setup` that leaves the machine alone and drives the app through it.
- Waits are named and bounded: `until(cx, "the window opened", deadline, …)` panics with "never: the window opened".
- See [rust/gpui.md](../rust/gpui.md) for the GPUI kit.

**Check.** A pull request that adds a window, a menu item, a command-line flag or a shortcut also adds a journey step. CI runs the journey on every system.

## One scenario per test, run everywhere

**What.** Each behaviour is one `#[test]`, written once and stamped out for every back end and system. A scenario a system can't run says so with a counted skip, never a silent pass.

**Why.** Crake once had one long chained test. Its first failure hid everything after it, and each OS had its own copy ([b32b88e](https://github.com/dorhawk-software/crake/commit/b32b88e)). The first runs of the split suite found a leaked network interface and a browser sign-in broken on two systems ([9a20b1d](https://github.com/dorhawk-software/crake/commit/9a20b1d)).

**Apply.**
- A macro lists the scenarios once and stamps them out per back end.
- Each test starts from a fresh fixture and removes everything it made.
- Differences per OS live in `platform/{linux,macos,windows}.rs`, not in the scenarios.
- A skip appends one line, `<scenario>: <why>`, to the file named by `PRACTICES_SKIP_LOG` (a passing test's output is hidden by `cargo test`, so a printed marker would be lost). `check-coverage` sets it and prints the count and the reasons after the tests. Crake's skips print "ok" and nothing counts them, which the shared version fixes.

**Check.** CI prints the skip count per system. A skip count that rises without a reason in the commit is a review question.

## Failures explain themselves

**What.** A failing test prints what the system decided and its state, not just "assertion failed".

**Why.** Two Windows CI failures in Crake came with nothing to go on. Afterwards the failing scenario dumps the daemon's decisions and the system state ([b92c65d](https://github.com/dorhawk-software/crake/commit/b92c65d)).

**Apply.** Give the fixture a `fail(what)` that prints the log, the relevant system state and the tool's full output, then panics.

**Check.** Read a failure from CI as if you had no machine to reproduce it on. Can you tell what went wrong?

## A step for every IPC call

**What.** Every call between processes (D-Bus, a socket protocol, a helper process) has a contract test. The real client talks to the real server over a private endpoint.

**Why.** A protocol change that only one side knows about passes every unit test ([17eca40](https://github.com/dorhawk-software/crake/commit/17eca40)).

**Apply.** The client takes its endpoint as a parameter, so the test needs no environment variables. Add one step per new call.

**Check.** A pull request that adds an IPC call also adds a contract step.

## Red before green

**What.** A fix comes with a test that failed on the old code. An important check is shown to fail under a deliberate mutation. The commit says how this was proven.

**Why.** A test that never failed proves nothing. Crake's first-connection fix was checked by bringing back the old wait ([f40f169](https://github.com/dorhawk-software/crake/commit/f40f169)). A platform check was run against an older release to show it could tell the two apart ([2ed1e85](https://github.com/dorhawk-software/crake/commit/2ed1e85)).

**Apply.** Run the new test on the old code first, or break the code on purpose. Write one line in the commit body: "fails on the old code, passes now".

Porting an agreed design is the one case where the code may come first: there is no old behaviour to fail on. Then prove each assertion by a mutation of the new code (a colour, a size, a branch). The commit lists the mutations, as Obálka's inbox did ([26ff3d5](https://github.com/dorhawk-software/obalka/commit/26ff3d5): six mutations, each failing a test).

**Check.** The commit body says how the test was shown to fail.

## No retries for flaky tests

**What.** A flaky test is never retried automatically. It prints everything when it fails. Each sighting is logged with its date and CI job, and the next diagnostic step is written down.

**Why.** A retry also hides the real failure that happens now and then ([d853dad](https://github.com/dorhawk-software/crake/commit/d853dad), [a1af142](https://github.com/dorhawk-software/crake/commit/a1af142)). Tests that hung on the clock or a race were fixed at the cause: fixed samples, waiting for the event instead of racing it ([3873cb8](https://github.com/dorhawk-software/crake/commit/3873cb8), [5b25a3c](https://github.com/dorhawk-software/crake/commit/5b25a3c)).

**Apply.**
- Tests don't depend on the clock, the screen size or the machine's devices ([1ba22e2](https://github.com/dorhawk-software/crake/commit/1ba22e2)).
- A retried wait prints each miss ([a4db81a](https://github.com/dorhawk-software/crake/commit/a4db81a)).
- Keep a "flakes seen" list in the testing doc, each entry dated with its job links.

**Check.** No `retry`, `--retries` or rerun-on-failure options in CI or the test runner.

## Real-machine bugs become tests

**What.** Keep a checklist of what real people have used the app for on real machines. Tick each item with the date and a role, never a name. A bug found there gets a test on the system where it appeared.

**Why.** VMs and CI prove the mechanics; people find the rest. Nearly every macOS fix in Crake came from a colleague's real Mac. One such bug became tests on two systems ([dc70963](https://github.com/dorhawk-software/crake/commit/dc70963)).

**Apply.** Add `docs/engineering/tried-by-people.md` with a line per platform and use case. Add the test in the same commit as the fix.

**Check.** A fix for a bug a person reported names its test in the commit body.

## Fakes refuse what they weren't told

**What.** A fake fails on any call the test didn't set up, and its message names the call. A fake HTTP server fails on a request it has no response for, naming the URL.

**Why.** A permissive fake hides calls nobody meant to make. In [Obálka](https://github.com/dorhawk-software/obalka), every server call is a legal act (signing in, marking mail delivered), so its fake transport throws "FakeTransport.X was called but not configured".

**Apply.** Build fakes from a table of configured answers; the default answer fails with the method's name. For TypeScript, see [typescript/testing.md](../typescript/testing.md).

**Check.** Remove one configured answer from a test: it fails, naming the call.

## A guard test per incident, proven able to fail

**What.** A bug that tests couldn't see where it happened (on a phone, in a release build, on another file system) gets a guard: a test that scans the source or config for the cause. Its header names the incident. The guard also checks that it scanned a real set of files, and runs itself against the original bad input.

**Why.** Obálka keeps about 40 such guards. One forbids a removed sign-in method from returning (owner decision, 2026-09-28). One forbids JavaScript globals the phone's engine lacks, after a backup restore shipped broken. One forbids file names that differ only by case ([3cbb48f](https://github.com/dorhawk-software/obalka/commit/3cbb48f)). The self-test against the original text caught a guard whose word boundary only worked in ASCII, on Czech text.

**Apply.**
1. Header: the incident, the date, the rule, the fix.
2. Assert the scan found more than a handful of files, so it can't pass on nothing.
3. Feed it the text that started it, and assert it fails.

**Check.** Each guard has a self-test with the original input.

## Golden fixtures written by the old code

**What.** A stored format (a backup, a database, a file) has fixtures written by the version of the code that first wrote that format, kept in the repository. Today's reader must open every one.

**Why.** A fixture written by today's code only proves today's writer agrees with today's reader. Obálka's fixture script checks out the commit for each schema version in a worktree and writes the fixture there. Its first two fixtures can't be regenerated: the history squash lost their commits, which its README says.

**Apply.** A script that takes a schema version, finds its commit, refuses if the version doesn't match, and writes the fixture. Record the commit, key and parameters next to the fixture. Use test keys that no secret scanner mistakes for real ones.

**Check.** One fixture per schema version, each with its source commit.

## Two implementations, one known answer

**What.** When the same format has two implementations (a native one and a fallback), both are tested against the same known answers, including output captured from the real device.

**Why.** An archive sealed on one phone must open on another build. Obálka uses its native crypto only after a launch-time check against the pure-JavaScript version, and its tests open bytes sealed by the native path on an emulator.

**Apply.** Store device-captured vectors as fixtures with their date and device. Run the known-answer check at start-up before trusting the fast path.

**Check.** The fallback opens every device-captured vector byte for byte.

## Coverage floors only rise

See [rust/coverage.md](../rust/coverage.md). The rule holds in any language: a floor per unit, raised as tests grow, never lowered to let a change through.

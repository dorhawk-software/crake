# Working rules for coding agents

A coding agent works on the developer's own computer, often while they use it, and sometimes overnight while they sleep. These rules keep the machine usable, the disk free, and the developer's time spent only on decisions that are really theirs. Each rule came from a real incident.

Put the rules a project adopts in its CLAUDE.md (or its agent instructions file), one line each, pointing here.

## Builds run at the lowest priority

**What.** Every build, test, lint, render or container job runs under `nice -n 19`. That includes `git commit` when a hook builds. Heavy jobs don't run side by side.

**Why.** Full-CPU builds stalled the developer's browser. With a VPN that encrypts in user space holding every DNS lookup, a busy CPU slowed every lookup (2026-10-15).

**Apply.** `nice -n 19 cargo test --workspace`, `nice -n 19 git commit …`.

**Check.** Review the agent's commands: does every heavy one start with `nice -n 19`?

## The disk keeps a floor

**What.** A hook after every agent command checks free space on the disk the builds go to.
- Under the floor, it clears the compiler's incremental caches.
- If that isn't enough, it fails and says what takes the room. The agent then stops and asks.

**Why.** Crake's build folder filled the developer's disk twice: 324 GB of old dependencies and incremental caches. The second time, two throwaway builds made it worse ([4f57555](https://github.com/dorhawk-software/crake/commit/4f57555), asked for 2026-10-19).

**Apply.**
- `.practices/bin/disk-guard` runs from `.practices/bin/claude-hook`.
- Set the floor in `.practices/config`, for example `DISK_FLOOR_GB=300`.
- It checks the disk of cargo's build directory, not the project's.

**Check.** The hook is in `.claude/settings.json` or the practices plugin. `df -h` before anything that builds a lot.

## Never a new target directory

**What.** No new `CARGO_TARGET_DIR` for worktrees, probes or one-offs. Check things in the main target, or with `cargo check`. If a separate one is truly needed, delete it in the same turn.

**Why.** Each target directory is a full build of every dependency, 5 to 15 GB for a GUI app. A Dependabot check in a worktree and a one-off probe added 20 GB to an already full disk.

**Apply.** Use `cargo check -p <crate>` instead of a probe build. Don't build to try out an `eprintln!`; find another way, or ask.

**Check.** `ls` the build disk after the session. Nothing new is there besides the project's own folder.

## Never `nix … path:` on a repo with a target folder

**What.** Evaluate or build a flake as `git+file://`, never as `path:`.

**Why.** A `path:` flake copies the whole folder into the Nix store, `target/` included, on every evaluation. One container doing it wrote 328 GB and filled the disk.

**Apply.** `nix build 'git+file:///src#default'`, which copies tracked files only. Watch `docker ps --size` on long containers.

**Check.** `grep -rn 'path:' scripts .github` finds no flake references.

## Builds go to the build disk

**What.** On a machine whose internal SSD is soldered in, cargo's intermediate files go to an external disk through `build-dir` in `~/.cargo/config.toml`. When the disk is unplugged, cargo fails on purpose.

**Why.** Rust builds wear an SSD that can't be replaced. Silently falling back to the internal disk would defeat the point.

**Apply.**
```toml
[build]
build-dir = "/Volumes/Build/cargo/{workspace-path-hash}"
```
Never work around the failure with `CARGO_BUILD_BUILD_DIR`, `CARGO_TARGET_DIR` or `--target-dir`. Write the code, then ask for the disk to be plugged in.

**Check.** `cargo metadata --format-version 1 | jq -r .build_directory` points at the external disk.

## Development builds aren't installed

**What.** Run the app from the build folder. Don't ask the developer to install units, policies or packages from a development build.

**Why.** "I don't want to poison my PC with interim development builds." Install-style instructions confused the developer and left things behind.

**Apply.** Give one copy-paste command that runs the binary from `target/debug`, and say plainly what to expect. Packages are tested in containers and VMs (see [testing.md](testing.md)).

**Check.** Review: does any instruction to the developer install something?

## Rebuild before saying "restart"

**What.** The last step of a change the developer will try is building the binaries they run. Check that they are newer than the changed sources, then say which process needs a restart.

**Why.** `cargo test` and `cargo clippy` don't refresh `target/debug/<app>`. An agent once said "restart to see it" while the binary was 40 minutes old.

**Apply.** `nice -n 19 cargo build -p <app>`, then `ls -l target/debug/<app>`.

**Check.** The handoff message names the binary and the process to restart.

## Release only on a green main

**What.** A release starts only after CI on main's latest commit has finished successfully on every system. Don't push anything else to main while the release waits.

**Why.** A release started 15 minutes after a push cancelled that push's CI. A new test then failed on Windows and macOS on the release commit (2026-10-14). Locally only one system's tests run.

**Apply.** The release script checks this and refuses. See [releases.md](releases.md).

**Check.** `gh run list --branch main --limit 1` shows `completed success` before a release starts.

## Quote measured CI timings

**What.** When telling the developer how long CI takes, quote the last runs' real timings.

**Why.** An agent said "about 40 minutes", which alarmed the developer. The real run took 16.

**Apply.** `gh run list --limit 5 --json createdAt,updatedAt,conclusion`.

**Check.** A duration in a message comes with where it was measured.

## Keep going on an agreed plan

**What.** Once a plan is agreed, work through every item in one go: commit, push, watch CI, start the next. Stop only for decisions that are really the developer's, such as licences, accounts, design choices and releases. Even then, carry on with the items that don't depend on them.

**Why.** "It is a shame you halted every time, I could tell you to continue because I was sleeping" (2026-10-20). The developer leaves work running overnight, and a halt wastes those hours.

**Apply.** Give short progress lines while working and one report at the end.

**Check.** A session that stopped mid-plan stopped for a question only the developer could answer.

## Screenshots don't flash the screen

**What.** Capture a window from its own pixels, for example X11's `XGetImage` through a small script. Never take bursts through the desktop's screenshot portal, which flashes the whole screen.

**Why.** The developer is using the screen. Headless captures (Xvfb, headless Chrome for mockups) need no screen at all.

**Apply.** Use the project's window-capture script for the app and headless Chrome for HTML. Delete full-screen captures after cropping.

**Check.** No more than one or two portal captures in a session.

## Long gates run in the main session

**What.** A gate that takes minutes (a simulator suite, a pixel match) runs in the agent's main session, one at a time. A sub-agent writes code and runs the quick checks; it hands the long gate back. An agent's work is done only once it has stopped: before a gate starts, no other agent still runs one.

**Why.** On 2026-11-21 a Scops sub-agent waited for its UI suite in the background and was stopped as stalled, twice, with its work uncommitted. An agent from that morning, whose work had been committed hours before, still ran its own suite under the shared lock.

**Apply.**
1. A sub-agent's brief names the long gates it leaves to the main session.
2. Before a long gate, list the running agents and test processes, and stop the orphans.
3. Run a gate in the foreground with a timeout, or in the background with its notification awaited; never in a sleep loop.

**Check.** Review: which agents run is visible only to the session that started them.

## Only the lock's own script waits for the lock

**What.** A shared lock is taken and waited for only by the script that owns it. That script can take over a lock whose holder is gone; a loop written around it can't.

**Why.** Scops' pixel match keeps a lock folder with the holder's process ID, and takes it over when that process is gone. On 2026-11-21 an agent waited for it in its own `until [ ! -d lock ]; do sleep 5; done`. A killed run left the lock behind, twice, and the loop waited for hours on a lock nobody held.

**Apply.** Call the gate's script and let it wait. To run two gates in turn, run them one after the other; never poll another gate's lock.

**Check.** todo: a PreToolUse claude-hook could refuse a command that polls a `*.lock` path in a loop.

## Simulators run one at a time until the machine is measured

**What.** A gate runs one simulator or emulator at a time, under its own
lock. Running several at once on the developer's machine waits until
load, temperature and memory are measured there, and found to have room.

**Why.** On 2026-11-23 three iOS simulators shot a screenshot gate at
once on a 64 GB M-series laptop. The shots fell from 395 s to 163 s. The
laptop reached a load average of 387, every core at 94–100 %, up to
103 °C, with 52 GB of memory in use; the run was stopped that day.
Xcode's own parallel testing was worse for this: it gave the whole test
class to one clone, and copied 3.1 GB per clone to the internal disk.

**Apply.**
1. One simulator at a time, under the gate's lock
   ([above](#only-the-locks-own-script-waits-for-the-lock)).
2. Before parallel runs become a default, measure them on the machine,
   with nothing else running: load average, temperature, memory. Try two
   devices, not three.
3. Parallel runs go to a machine with the room for them, such as CI.
4. Write the measurement and the decision down
   ([not-now.md](not-now.md)).

**Check.** todo: claude-hook could refuse booting a second simulator
while one is booted (`xcrun simctl list devices booted`).

## Don't edit a script while it runs

**What.** A script that a run is executing isn't edited until the run
ends. Wait, or stop the run first.

**Why.** bash reads a script as it runs, line by line. On 2026-11-23 a
screenshot gate's `run.sh` was edited during a run. The run died with
"syntax error near unexpected token" after minutes of shots it had
already taken.

**Apply.** Before editing a gate's script, check that no run of it is
going (`pgrep -fl run.sh`). A script whose body is one function, called
as `main "$@"; exit` on its last line, is read whole before it starts.

**Check.** todo: claude-hook could refuse an edit to a shell script that
a running process executes (`pgrep -f <path>`).

## Write down what is deferred

See [not-now.md](not-now.md). Whatever is decided "not now" is written in the repo, in the same commit.

# Rust: code conventions

One workspace, lints and versions set once at the top, all unsafe code behind one safe layer, and OS code behind small modules so the decisions stay portable. Errors are sentences a user can read. Files and functions stay small enough to hold in your head.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## One workspace, set once

**What.**
- The workspace uses resolver 3 and edition 2024.
- `[workspace.package]` sets `version`, `edition`, `license` and `publish = false`, and every crate inherits them.
- Third-party versions live once, in `[workspace.dependencies]`.
- `[workspace.lints]` is shared (see [lints.md](lints.md)).
- Each dependency gets a comment saying why it's there.

**Why.** A version repeated in several crates drifts. An app's crates aren't published, so `publish = false` prevents an accident ([0b63fbb](https://github.com/dorhawk-software/crake/commit/0b63fbb)).

**Apply.**
```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"
publish = false

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
```
In a crate: `serde.workspace = true`.

**Check.** `grep -h '^[a-z_-]* = "' crates/*/Cargo.toml | sort | uniq -d` finds no third-party crate versioned in two crates.

## Unsafe code in one crate

**What.**
- Every `unsafe` block lives in one crate (for example `<app>-os`), behind safe functions, each with a `// SAFETY:` comment.
- Every other crate root says `#![forbid(unsafe_code)]`.
- `.practices/bin/check-unsafe` also catches what a `cfg` hides from the compiler, as well as tests and build scripts.

**Why.** "A review of what could break memory safety is a review of this crate" (asked for 2026-10-14; [91388a7](https://github.com/dorhawk-software/crake/commit/91388a7), [ffff883](https://github.com/dorhawk-software/crake/commit/ffff883)). A crate can't partly override workspace lints, so the attribute plus the grep is the design.

**Apply.**
- Set `UNSAFE_PATHS="crates/app-os"` in `.practices/config`; leave it empty for "no unsafe anywhere".
- `std::env::set_var` is unsafe in edition 2024. It goes in the OS crate too. Tests stop setting environment variables, because that races with the test threads; pass the value in instead.

**Check.** `.practices/bin/check-unsafe` in `lint`.

## OS code behind small modules

**What.**
- Decisions are portable. Everything that touches the OS lives in `sys/{linux,macos,windows}.rs` (and `unix.rs`).
- `sys/mod.rs` declares each module under `#[cfg]` and re-exports it.
- An OS with nothing to do gets a `#[cfg(not(...))]` stub whose doc comment says why. A stub that should fail fails loudly; it doesn't return a silent `false`.
- OS-only dependencies go under `[target.'cfg(...)'.dependencies]`.

**Why.** It is what made Crake's Windows and macOS ports a matter of new modules, not rewrites ([829b7cd](https://github.com/dorhawk-software/crake/commit/829b7cd)).

**Apply.** Move tray, notifications, keyring and process helpers into `sys/`. Add a CI `cargo check --target x86_64-pc-windows-gnu` before the port starts, so it stays compilable.

**Check.** `rg '#\[cfg\(target_os' --glob '!**/sys/**'` finds little outside `sys/`, and each hit is a review question.

## Errors are sentences

**What.**
- An error the user may see reads as a finished sentence in the product's voice.
- Library crates return error enums whose `Display` is that sentence. The daemon or CLI adds context with `anyhow`.
- Errors crossing a process boundary keep their wording; nothing like "org.freedesktop.DBus.Error.Failed:" is put in front.
- In a translated app the library returns an enum, and the UI turns it into words through its translations. Strings are never built in the library.

**Why.** zbus 6 put its error name in front of the text users saw ([76efb39](https://github.com/dorhawk-software/crake/commit/76efb39)).

**Apply.** `bail!("The profile names no server, so there's nothing to connect to.")`. In the UI, use `Result<T, Refusal>` with a `said()` or a translation key.

**Check.** A review question: would a user understand this message without the code?

## Logs to stderr

**What.** Use `tracing` with `EnvFilter`, defaulting to `info`. Write to stderr, with ANSI colours only when stderr is a terminal.

**Why.** launchd keeps only stderr, and a Mac's logs were empty for days. The systemd journal showed escape codes around every level ([450bf93](https://github.com/dorhawk-software/crake/commit/450bf93)).

**Apply.**
```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
    .init();
```

**Check.** Run the service under the OS's supervisor and read its log.

## Config refuses what it doesn't know

**What.** Config structs use `#[serde(deny_unknown_fields)]`. Defaults come through `#[serde(default = "fn")]`. Paths per OS are constants under `#[cfg(unix)]` / `#[cfg(windows)]`.

**Why.** A misspelled key that is silently ignored looks like a bug in the app.

**Apply.** Put the attribute on every struct read from a file.

**Check.** A test reads a file with an unknown key and expects a refusal that names it.

## Small files and functions

See [lints.md](lints.md#size-limits). Split a folder module by topic, one file per part, with a sibling `tests.rs` for its tests. Every module starts with a `//!` line saying what it is.

## Magic numbers have names

**What.** A number in a crate's own code (not its tests) has a name: a `const`, a `static` or an enum discriminant. Otherwise it carries `// literal: <reason>` on its line or the line above. 0, 1 and 2 are free.

**Why.** Asked for 2026-10-12. Clippy has no such lint. Crake named 633 numbers across two crates ([8676f1b](https://github.com/dorhawk-software/crake/commit/8676f1b), [005c7fc](https://github.com/dorhawk-software/crake/commit/005c7fc), [aa510d2](https://github.com/dorhawk-software/crake/commit/aa510d2)).

**Apply.** Crake's `lint-literals` tool becomes a published crate in a later phase. Until then, adopt it crate by crate through `.practices/lint.d/`. Shared units (`SECS_PER_MINUTE`) go in a `units.rs`.

**Check.** The linter in `lint.d`.

## Windows: the C runtime linked in

**What.** `.cargo/config.toml` sets `+crt-static` for both MSVC targets, and CI fails if a built `.exe` still names `VCRUNTIME140`.

**Why.** A fresh Windows 11 has no VCRUNTIME140.dll, and the app didn't start ([897d900](https://github.com/dorhawk-software/crake/commit/897d900)). CI runners have the runtime installed, so running the programs there doesn't catch it.

**Apply.**
```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
[target.aarch64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

**Check.** In CI: `Select-String -Path *.exe -Pattern VCRUNTIME140` finds nothing.

## Measure before claiming numbers

**What.** Performance claims come with a measurement and how it was taken.

**Why.** Crake recorded "performance: never measured" as an open item rather than guess. Its measured fixes come with numbers: 500 rows went from 20 ms a frame to under 2 ms.

**Apply.** See [gpui.md](gpui.md#frame-timing) for the frame-timing recipe. Expect about ±30% noise between runs; compare medians of several.

**Check.** A commit that claims "faster" has a number in it.

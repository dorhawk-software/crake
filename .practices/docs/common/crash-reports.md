# Crash reports and logs

What may leave the device, and when. Crash reporting is only sent after
the user agrees, through one scrubber the project has checked in the
vendor's own code, and expected refusals aren't reported at all.

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka),
an app whose data (official government mail) must never leave the phone.

## Consent first: nothing starts before the user agrees

**What.** The crash-reporting SDK is not initialised until the user agrees.
"Off" means not started, not started-and-filtered.

**Why.** Obálka found that its SDK passes the native SDKs an options object
with every callback stripped. The native crash handler and release-health
pings ignored the consent check that the JavaScript side applied.
Nothing escaped only because the reporting address was still empty.

**Apply.** Call `init` from the consent screen's "yes", and on launch only
when consent is stored.

**Check.** Fresh install, no consent: a network capture shows no request
to the crash service.

## Check every hook in the vendor's code, not its docs

**What.** When privacy depends on an SDK's hook (a `beforeSend`), read the
SDK's source to find every path data takes. Cover each one, or turn it
off.

**Why.** "One interception point" was a claim about someone else's code.
In Obálka, `beforeSend` covered error events only. Transactions,
including an HTTP span for every server call, skipped both the consent
check and the scrubber.

**Apply.** List the SDK's event kinds from its source: errors,
transactions, sessions, breadcrumbs, native crashes. Set the hook for
each, or disable the kind (Obálka turned automatic HTTP spans off).

**Check.** A test sends one event of each kind through the configured
client and asserts what reaches the transport.

## An allow-list scrubber, seeded with what the device knows

**What.** Reports go through a scrubber that keeps known-safe fields and
removes the rest. It is seeded at start-up with the identifiers the device
holds (account IDs, logins, names) and removes them wherever they appear.

**Why.** A deny-list of patterns misses the value nobody thought of. The
device already knows its users' identifiers, so it can remove them
literally.

**Apply.** Allow-list per field; then a literal pass for every seeded
identifier, in every string.

**Check.** A test feeds a report holding each seeded identifier; none
survives.

## Three tiers, three rules

**What.** Data leaving the code has three destinations, each with its own
rule:
1. **Crash reports** leave the device: an allow-list, after consent.
2. **A debug bundle** the user exports and owns: rich, with a deny-list for
   credentials.
3. **The console**: a deny-list, through one logger; no direct `console`
   calls.

**Why.** One rule for all three is either too strict to debug with or too
loose for a report that leaves the phone. Obálka's audit of 2026-09-10
found 84 catch blocks and none of them reporting anything.

**Apply.** One module per tier. A test forbids network APIs in the debug
tier's folder, so a debug bundle can never be uploaded.

**Check.** Each tier has its module and its test.

## Expected refusals aren't reported

**What.** A failure the OS returns for a normal reason (a locked phone, a
permission not yet answered) is handled as "not now" and not reported.
Every other failure is.

**Why.** Obálka's crash reports filled with Keychain errors from locked
phones, hiding real failures
([451fe8f](https://github.com/dorhawk-software/obalka/commit/451fe8f)).
See [mobile/devices.md](../mobile/devices.md#expected-refusals-from-the-os-arent-reported).

**Apply.** Match the exact code; report all others with the operation's
name.

**Check.** A test per refusal, and one showing a different code still
reported.

# Install it and it just works

The owner (2026-11-13): "what all my apps are about is basically
'install it and it just works'". Every Dorhawk product is built to that
promise. The product makes the decisions a user would otherwise have to
make, says what it decided, and lets them undo it.

## The product decides, not the user

**What.** From installation to first success, the user makes no
technical choice. Whatever the product can decide from a signal, it
decides itself. It says each decision in one sentence (what, and the
fact that triggered it), and the user can undo it.

**Why.** Crake finds what a VPN needs by itself. The Windows-app runner
started from what the owner disliked in CrossOver: advanced settings
asking them to pick a graphics layer, DLSS, MSync or high-resolution
mode ("quite confusing"), and moving Enterprise Architect to another
display and resizing it to get full screen ("all in all quite
cumbersome"). Each switch moves the product's job to the user.

**Apply.**
1. List every choice a user meets from download to first success: the
   installer, permissions, first run, and every setting.
2. For each one, ask whether a signal could decide it: the system, the
   file, what happened on the last try. If one can, the product decides,
   with a sensible default when no signal exists.
3. Tell the user about the decision in one plain sentence, with the
   reason, in the place they'll look (a step in a story, a line in the
   log), and offer Undo.
4. Check the outcome the user cares about, not only that the step ran.
   For example, the window fits the screen and the text is sharp, not
   just "the setting is on".
5. Check every surface the user sees, not only the main one: the
   splash, dialogs, sheets, error windows, the first and the second
   launch. CrossOver shows Enterprise Architect's main window at the
   right size and its splash huge: the first thing the user sees says
   nobody looked (owner: "shows not enough attention to detail").

**Check.** A fresh install on a clean user account reaches first
success with no choices beyond what the operating system demands.
Count the steps and choices on each release; the number never grows
unnoticed.

## Settings are preferences, not workarounds

**What.** A setting earns its place only where people genuinely differ:
telemetry, language, where files go, appearance. A technical workaround
(a renderer, a sync mode, a compatibility flag) is never a setting. It
is the product's job to choose it. Experts see and override decisions
in the log, on the command line or in a config file, never as switches
in the window.

**Why.** Every technical switch in the window is a question most users
can't answer, and a wrong answer looks like the product failing.
CrossOver's "Advanced settings" are the example to avoid.

**Apply.**
1. Sort every setting into "preference" or "workaround". Remove the
   workarounds from the UI and make each one a decision (above).
2. Give each preference a default that suits most people, so first use
   needs none of them.
3. Keep the expert path: every decision shows in the technical log, with
   a way to override it outside the main window.

**Check.** The Settings window holds only preferences. Nothing in it
needs a technical term to explain.

## What it leaves behind says whose it is

**What.** Every folder or file the product puts where the user looks
(Documents, the Desktop, Downloads, the home folder) is named after the
product, or sits inside a folder that is, and carries its icon where the
system allows. Someone who finds it a year later, with no memory of
making it, can tell which product it belongs to and that it holds their
work.

**Why.** The Windows-app runner's board recommended giving each app a
folder "Documents › Enterprise Architect". The owner: "a person could be
like 'hmm, i dont remember needing any documents for EA' and just
deleting the directory". A folder nobody remembers making is a folder
someone deletes, with the work inside it. Documents › the product ›
the app keeps the work findable, backed up and synced, and says whose
it is.

**Apply.**
1. List every path the product creates outside its own hidden places
   (Library, caches): on install, on first run, and per item.
2. Give each one a parent folder named after the product, and the
   product's icon on that folder where the platform allows it.
3. Read each path cold, as someone tidying up a year later: does it say
   who made it and what is inside?

**Check.** After a full run of the product, every new item outside its
hidden places sits under one folder with the product's name.


# Feedback while it works

How an app shows that it heard a press, and that the work is under way.
The motion itself follows
[Motion with a reason](visual-craft.md#motion-with-a-reason): the
product's own, played while the work runs, with a still frame for
reduced motion.

A board marks a control in its busy state with the standard attribute
`aria-busy="true"`. `check-ui` measures it there, as
[visual-craft.md](visual-craft.md) describes. A deliberate exception is
marked `data-allow="busy: the reason"`.

## Work shows it's happening

**What.** Anything that takes longer than about a second shows motion
where the person is looking. The motion starts with the work, stops when
the result shows, then rests. It is the product's own, not a generic
spinner. With reduced motion it rests on a still frame, and words in
progress form ("Installing…") still say it's working.

**Why.** Gentes, the Windows-app runner, spends minutes getting an app
running. It showed a still egg and a still story the whole time. The
owner, 2026-11-18: "there is no animation whle something is hapepning,
do practices not say anything about animations displaying progress?"
They didn't: "Motion with a reason" said when a loop may play, not that
work must show one. Fixed in Gentes commit 01674e5 (a private
repository). The speckle on the egg draws itself in, fills and fades
every 2.8 s while it works, and the story's live mark fills with it.
Both rest on a still frame with Reduce Motion.

**Apply.**
1. List every wait over about a second: installing, opening, signing
   in, downloading, saving.
2. Put each one's motion where the person looks: on the thing that is
   working, or on the control they pressed.
3. Start the motion with the work and stop it on the result, never on
   a timer. A timer is wrong whenever the work is faster or slower.
4. One motion per screen. Two things moving at once compete for the
   eye.
5. With reduced motion, show a still frame of the same motion, and
   words in progress form beside it.
6. On a board, draw the working state with its motion, and the still
   frame under `@media (prefers-reduced-motion: reduce)`.

**Check.** `check-ui` (working) loads each board with a busy control
again, with motion allowed. Something inside each element marked
`aria-busy="true"` must be animating. `check-ui` (motion) renders every
board with reduced motion, and nothing may still loop. Whether each wait
in the app has its motion, and whether the motion is the product's own,
is review.

## A press answers at once

**What.** A control pressed for something that takes time changes in
the same frame. It names what is happening, ending in "…" ("Opening
Enterprise Architect…"), and shows the work's motion. It takes no more
presses, and its shortcut and menu item are off too. It stays so until
the outcome is visible, or until a time limit passes; then it returns.
The limit is measured from the slowest real case.

**Why.** Gentes' "Open Enterprise Architect" button gave no sign for the
10 to 30 seconds Wine takes to show the app's window. So it could be
pressed again and again. The owner, 2026-11-18: "when I click on this
butron could it change to something like "launching", be disabled and
with a rotating progress or something? because right now it gives no
feedback and a person could click it multiple times". Fixed in Gentes
commit 0fbea69. The button reads "Opening Enterprise Architect…" and
shows the egg's forming speckle where its icon was. It takes no clicks,
and its ⌘↩ shortcut is off. It returns when Gentes sees the app's window
(it looks every 0.5 s), or after 90 s. A window test with a stand-in app
that takes a second to show proves it. The product's own motion took the
place of the "rotating progress" (Motion with a reason).

**Apply.**
1. Put the busy state on the control that was pressed, not in a toast
   or a status line somewhere else.
2. Change it in the same frame as the press, before the work starts.
3. Name the work in progress form, ending in "…" (U+2026), never three
   dots: "Opening Enterprise Architect…", "Installing…".
4. Show the work's motion in the control, where its icon was.
5. Turn off the control, its keyboard shortcut and its menu item. A
   second press does nothing.
6. Return it when the outcome itself is visible: the window shows, the
   file is saved. Not when a step before it ends.
7. Give it a time limit, from the slowest real case measured, with room.
   When the limit passes, the control returns.
8. Write a journey test ([common/testing.md](../common/testing.md#a-journey-suite-is-the-regression-suite)).
   It holds the work open with a slow stand-in. It asserts the busy
   state, that a second press does nothing, and that the control returns.
9. On a board, draw the pressed control with `aria-busy="true"` and
   `disabled` (or `aria-disabled="true"`).

**Check.** `check-ui` (busy) finds every element marked
`aria-busy="true"` on the boards. It, or the control inside it, must be
`disabled` or `aria-disabled="true"`, and neither may show a pointer
cursor. Its words must end in "…"; three dots are a finding too.
`check-ui` (working) then asks for motion inside it. Whether each press
in the app has its busy state and its journey test is review.

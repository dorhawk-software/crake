# Dates, times and numbers

How a person enters a value the app works with: a date, a time, a count.
The lessons come from Scops, a baby-sleep app, on 2026-11-15. Its
onboarding typed a birth date, capped the nap count, and offered only
recent years. The drawing rules are in [visual-craft.md](visual-craft.md).

## Picked, not typed

**What.**
- A date, time or count is chosen with a picker that fits it, never
  typed into a free text field.
- Everything worked out from it updates at once.
- Pickers offer only valid values: no birth date in the future, no
  one-off event in the past.

**Why.** Scops' onboarding asked for the birth date in a plain text
field. In the prototype the owner typed a new date, and "He is 9 months
and 25 days" stayed as it was. That age was wrong in the design itself:
20 January to 17 November is 9 months and 28 days.

The owner found the typed date. The wake-up time on the same screen was
still a lone toggle ("6:30" turning green on tap). Four more places
typed or only displayed times: event times, the night correction, six
day-plan values and the attempt limits. The owner had to find the second
case. A search for time-shaped values across all boards found the rest
in seconds.

**Apply.**
1. List every date, time and number the user enters.
2. Give each the matching picker: a month calendar for dates, a stepper
   for small counts, a time wheel for times.
3. Show the picked value on a button with an icon, so it looks tappable.
4. Next to it, show what is derived ("He is 10 months and 4 days"), and
   recompute it when the value changes.
5. Work out every derived example on the board, by hand or in code.
6. When one entry turns out wrong, search the whole design for the same
   kind and fix them all in one pass:
   - every typed time or date;
   - every lone toggle that really holds a value;
   - every value shown as plain text that should be editable.

**Check.** In the prototype, change each value. Every derived text
changes with it, and the arithmetic holds. A search of all boards for
time- and date-shaped text finds no typed or display-only value.

## Every calendar jumps to a month and year

**What.** In any calendar, the month's name is a button. It opens a year
and month view, so nobody arrows back through months to reach a date
years ago. The arrows stay for the next and previous month.

**Why.** Scops' birth-date picker had only arrows. The owner: in a
calendar where you go thirty years back, arrowing left and right is a
real pain. It holds for every calendar, not just this one.

**Apply.**
1. Draw the month title as a button with a chevron ("January 2026 ▾").
2. Tapping it shows the years and a 3 × 4 grid of months. "Show August
   2025" returns to the days.
3. Lead with the likely years: for a baby, the last four. Let the view
   page further ([below](#suggestions-are-a-starting-point-never-a-fence)).
   For long ranges, use a scrolling year list, or let people type the
   year.
4. Disable months and days outside the allowed range, in both views.
5. The confirm button names the exact choice ("Pick 14. 8. 2025").

**Check.** From the default month, reach a date three years back in at
most four taps. A disabled month cannot be picked.

## A calendar shows the neighbouring months' days, muted

**What.**
- A month grid fills its first and last week with the days of the
  previous and the next month. They are muted, but tappable: tapping
  one switches the month and picks that day.
- Days that cannot be picked look different again (fainter), so the two
  kinds are never confused.

**Why.** Scops' birth-date calendar left October's first Monday and
Tuesday empty. The owner expected 29 and 30 September there, and 1 and
2 November at the end. In the event calendar, the first fix made "past,
can't pick" and "next month, can pick" look the same. That is a new
double meaning ([visual-craft.md](visual-craft.md#one-reading-only)).

**Apply.**
1. Fill whole weeks: the leading days of the previous month, the
   trailing days of the next.
2. Draw three looks, all different:
   - this month: a tile;
   - a neighbouring month: no tile, muted;
   - not allowed: no tile, fainter still, and disabled.
3. Tapping a neighbouring day moves to its month and selects it.

**Check.** Every row of the grid is a full week. On one screenshot, a
newcomer can tell the three kinds of day apart.

## Suggestions are a starting point, never a fence

**What.** Wherever the design offers likely choices (the last four
years, the usual nap count, common event types), there is always a way
past them. Likely choices make the common case fast. They never decide
what is possible.

**Why.**
- Scops' year view offered only 2023 to 2026, because its users have
  small children. The owner: the design must always let people go
  "outside" the suggestions. Older siblings use the app too.
- The nap count was capped at three earlier. The owner asked for more.
- Going past the suggestions showed a follow-on bug: the age read "221
  months". Past 24 months, the age now switches to years and months.

**Apply.**
1. For every set of offered choices, add the way out:
   - arrows that page further;
   - "Other", with free input;
   - a stepper without a hard cap.
2. Only real limits disable a choice: no birth in the future, no event
   in the past. A guess about the audience never does.
3. Check what is derived at the far ends: years instead of months,
   plural forms.

**Check.** For each picker, try a value far outside the suggestions (a
birth 18 years ago, five naps). It can be entered, and everything
derived from it reads well.

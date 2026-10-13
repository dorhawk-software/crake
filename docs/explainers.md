# To explain, with pictures

How Crake decides is not obvious, even to people who use VPNs every day.
These are the ideas worth explaining for users (and helpdesks): each in
plain words, with a diagram or a short animation, in Crake's voice
(`design/DESIGN.md`, Copy voice). Where they end up is open: a "How it
works" page on dorhawk.software/crake, the README, help cards in the app.
Each section says what to explain, the picture it needs, and the words to
start from.

## 1. DNS, the internet's phone book

Every website is looked up by name first, to get an address. Crake works
with those lookups, so everything below starts here.

- **Picture:** a laptop asking "wiki.acme.internal?" and a phone book
  answering with an address; the public phone book shrugging "never heard
  of it", the company's answering.
- **Words:** "Before your computer connects anywhere, it looks the name
  up. Public DNS knows public names; only your company's DNS knows its own."

## 2. Two signs that a host lives inside

What makes Crake send a host through a tunnel, and nothing else does.

- **A name only the company knows:** public DNS has never heard of it,
  the company's DNS has. Routed before the first packet.
- **A host that doesn't answer:** no answer directly within the wait (3 s
  by default), an answer through the tunnel. Then just that host goes
  through.
- **Picture:** two lanes, "direct" and "through ACME", for each case (the
  website's "How it decides" animations are a start).

## 3. Watching from the side, or standing in the middle

Why Crake can't take the internet down on Linux, and what it does about
that on macOS and Windows.

- **Linux:** the system asks its own DNS directly; Crake watches the
  answers go past in the firewall and steps in only when one says "no such
  name". If Crake stops, lookups carry on untouched.
- **macOS and Windows:** there's no way to watch from the side, so Crake
  stands in the middle: it tells the computer "I'm your phone book now"
  and passes each question on to the real one.
- **The safety nets:** Crake stands in only when it knows the real phone
  book; when that stops answering (a hotel's sign-in page, a network that
  changed), Crake steps aside after three unanswered questions, says so,
  and tries again two minutes later.
- **Picture:** side by side, Linux with Crake beside the path (a magnifying
  glass on the line), macOS/Windows with Crake on the path; then the same
  with the real phone book silent, and Crake stepping out of the way.

## 4. Does it slow anything down?

- Traffic that isn't the company's goes straight out, at full speed:
  usually faster than a VPN that sends everything through the company.
- On macOS and Windows each lookup takes one extra hop inside your own
  computer (a fraction of a millisecond); most are cached and never reach
  Crake.
- A little extra only where Crake is helping: a name public DNS doesn't
  know is asked of the company's DNS (tens of milliseconds, and not again
  for 5 minutes); the first connection to a company host known only by its
  address can wait for the probe (Linux usually avoids even that).
- **Picture:** a timeline of one lookup, with and without a tunnel up.
- **To do first:** measure it (lookup times with and without a tunnel, on
  each system), so the page states numbers rather than estimates.

## 5. Crake or a VPN that takes everything

- **Crake:** calls, streaming and downloads go straight out; the company
  sees only traffic to its own hosts; printers and the home network keep
  working; nobody has to keep a list of company networks up to date.
- **A full tunnel is still right when** IT wants all traffic inspected
  (security filtering, compliance), or for extra cover on untrusted Wi-Fi.
  Crake does that too when IT says so (`full_tunnel = true`), and says in
  the panel that it won't learn there.
- **Picture:** two maps of the same morning: every request travelling via
  the company, against only the company's going there.

## 6. The first connection

How even the first connection to a host that needs the tunnel works on
Linux (the first SYN held while Crake races direct against the tunnels),
and why on Windows the first one fails and the retry works.

- **Picture:** a stopwatch: the connection held for a moment, a direct
  try and a tunnel try racing, the winner taking the connection.

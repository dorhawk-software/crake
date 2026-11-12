---
name: practices-releases
description: Cut, rehearse or fix a release the dorhawk practices way - only a green main is released, one command from Unreleased to the tag, the workflow re-checks then publishes in a safe order, a rehearsal that publishes nothing, honest timeouts for slow external steps. Use when the person asks to release, tag, publish, ship a version or write release notes.
---

# Releases

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

Read `common/releases.md` and `common/changelog.md` (the changelog is
the release notes), then the project's own release notes in
`docs/engineering/releases.md` when it has them.

## Musts

- Check that main is green in CI before anything. Never release a red or
  unpushed main.
- Rehearse first when the project has a rehearsal.
- The Unreleased section becomes the version's section; the release
  commit's subject is just the version.
- Tags and publishing are outward-facing: confirm with the person before
  pushing a tag.

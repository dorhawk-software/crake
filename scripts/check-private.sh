#!/usr/bin/env bash
# Nothing customer-specific gets committed: refuse files (as staged) and
# commit messages that match any pattern in dev/private-terms, a git-ignored
# list of the user's clients' names, hosts and addresses.
#
#   scripts/check-private.sh            # the files as they would be committed
#   scripts/check-private.sh MSG_FILE   # a commit message (commit-msg hook)
#
# Used by scripts/lint.sh and scripts/pre-commit.sh, and as git's
# commit-msg hook:
#   ln -s ../../scripts/check-private.sh .git/hooks/commit-msg
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
terms=dev/private-terms
[ -f "$terms" ] || { echo "check-private: no $terms here, nothing to check against" >&2; exit 0; }
patterns=$(grep -v -e '^#' -e '^[[:space:]]*$' "$terms")
[ -n "$patterns" ] || exit 0
args=()
while IFS= read -r p; do args+=(-e "$p"); done <<<"$patterns"
# grep -P takes one pattern: all of them as one alternation.
one=$(sed 's/.*/(?:&)/' <<<"$patterns" | paste -sd'|')

if [ $# -ge 1 ] && [ -f "$1" ]; then
    if grep -v '^#' "$1" | grep -P -i -n -e "$one" >&2; then
        echo "check-private: the commit message names a client (see above)." >&2
        exit 1
    fi
    exit 0
fi

# Staged content, plus tracked files as they are now.
found=0
git grep --cached -P -i -n -I "${args[@]}" -- ':!vendor' >&2 && found=1
git grep -P -i -n -I "${args[@]}" -- ':!vendor' >&2 && found=1
# New files not yet added.
mapfile -d '' new < <(git ls-files --others --exclude-standard -z)
if [ ${#new[@]} -gt 0 ]; then
    grep -P -i -n -H -I -e "$one" -- "${new[@]}" >&2 && found=1
fi
if [ $found = 1 ]; then
    echo "check-private: client names, hosts or addresses above must not be committed." >&2
    exit 1
fi
exit 0

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
#
# git grep does every search, as its -P is PCRE on every system (BSD
# grep, macOS', has no -P), and what it can't search is refused, never
# taken as clean: until 2026-10-26 this script used GNU grep -P, paste
# and bash 4's mapfile, and on a Mac it let every commit through.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
terms=dev/private-terms
[ -f "$terms" ] || { echo "check-private: no $terms here, nothing to check against" >&2; exit 0; }
args=()
while IFS= read -r p || [ -n "$p" ]; do
    [[ $p == '#'* || $p =~ ^[[:space:]]*$ ]] && continue
    args+=(-e "$p")
done <"$terms"
[ ${#args[@]} -gt 0 ] || exit 0

found=0
# One search, git grep's options up to `--`, then the paths: 0 found,
# 1 nothing, anything else couldn't search: refused.
look() {
    # Never empty: bash 3.2 (macOS') calls an empty array unbound.
    local opts=(-P -i -n -I)
    while [ $# -gt 0 ] && [ "$1" != -- ]; do
        opts+=("$1")
        shift
    done
    git grep "${opts[@]}" "${args[@]}" "$@" >&2
    case $? in
        0) found=1 ;;
        1) ;;
        *)
            echo "check-private: couldn't search (above), so nothing may be committed." >&2
            exit 2
            ;;
    esac
}

if [ $# -ge 1 ] && [ -f "$1" ]; then
    # Without its comments, in a folder of its own: git grep searches no
    # file outside the folder it starts in.
    msg=$(mktemp -d)
    trap 'rm -rf "$msg"' EXIT
    grep -v '^#' "$1" >"$msg/message"
    (cd "$msg" && git grep --no-index -P -i -n -I "${args[@]}" -- message) >&2
    case $? in
        0) echo "check-private: the commit message names a client (see above)." >&2 ;;
        1) exit 0 ;;
        *) echo "check-private: couldn't search the commit message (above)." >&2 ;;
    esac
    exit 1
fi

# Staged content, plus tracked files as they are now.
look --cached -- ':!vendor'
look -- ':!vendor'
# New files not yet added.
new=()
while IFS= read -r -d '' f; do new+=("$f"); done < <(git ls-files --others --exclude-standard -z)
[ ${#new[@]} -eq 0 ] || look --no-index -- "${new[@]}"
if [ $found = 1 ]; then
    echo "check-private: client names, hosts or addresses above must not be committed." >&2
    exit 1
fi
exit 0

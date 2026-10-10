#!/usr/bin/env bash
# Claude Code hook (PostToolUse, Bash): after a `cargo build` or `cargo test`,
# run scripts/lint.sh. A failure exits 2, which Claude Code hands back to
# Claude as an error to fix before going on.
set -uo pipefail
input=$(cat)
command=$(printf '%s' "$input" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input",{}).get("command",""))' 2>/dev/null)
case "$command" in
    *"cargo build"* | *"cargo test"*) ;;
    *) exit 0 ;;
esac
if ! out=$("$(dirname "$0")/lint.sh" 2>&1); then
    printf 'Lint gate failed: fix these, then build again.\n\n%s\n' "$out" >&2
    exit 2
fi
exit 0

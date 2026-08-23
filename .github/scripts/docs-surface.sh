#!/usr/bin/env bash
# docs-surface.sh: report public surface added by a diff that no document mentions.
#
# Never fails the build. It prints findings and exits 0, because the point is to
# notice drift, not to block a merge. Run it in CI on a pull request, or by hand
# during a documentation round:
#
#   .github/scripts/docs-surface.sh origin/master        # working tree vs master
#   .github/scripts/docs-surface.sh origin/master pr35   # an explicit head ref
#
# What it checks, and the falsifier for each:
#   1. every `pub` item added under musecode_core/src is named in docs/API.md,
#      inside a code span or a fenced block rather than anywhere in the prose
#   2. every new module file has a row in docs/ARCHITECTURE.md and in the
#      CLAUDE.md layer table
#
# Both are set differences over a diff, which is the kind of check a reader
# misses and a script does not. Neither judges whether the prose is any good.
#
# The documents are read at the same revision as the diff. With an explicit head
# ref the docs come from that ref, not from whatever the working tree happens to
# hold, or the script reports drift that the branch already fixed.

set -uo pipefail

BASE="${1:-origin/master}"
HEAD_REF="${2:-}"
EXPLICIT_HEAD="$HEAD_REF"
: "${HEAD_REF:=HEAD}"

SRC_PREFIX="musecode_core/src"
API_DOC="docs/API.md"
ARCH_DOC="docs/ARCHITECTURE.md"
LAYER_DOC="CLAUDE.md"

if ! git rev-parse --verify -q "$BASE" > /dev/null; then
  echo "docs-surface: base ref '$BASE' not found, nothing to compare against"
  exit 0
fi

RANGE="$(git merge-base "$BASE" "$HEAD_REF")..$HEAD_REF"
findings=0

# Read a document at the revision the diff describes. Without an explicit head
# ref the working tree is the revision, which is what the by-hand round wants.
read_doc() {
  if [ -n "$EXPLICIT_HEAD" ]; then
    git show "${HEAD_REF}:$1" 2>/dev/null
  else
    cat "$1" 2>/dev/null
  fi
}

# Keep only code spans and fenced blocks. A name that appears solely in prose is
# a coincidence, not documentation: `hit` and `mark` are ordinary English words.
code_only() {
  awk '/^```/ { f = !f; next } f { print; next } /`/ { print }'
}

emit() { # emit <file> <message>
  echo "::warning file=$1::$2"
  echo "- \`$1\`: $2" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  findings=$((findings + 1))
}

# 1. public items added in this range. Hunks whose context names a test module
# are skipped: a pub helper inside `mod tests` is not public surface.
added_pub=$(git diff --unified=0 "$RANGE" -- "$SRC_PREFIX" \
  | awk '
      /^@@/      { ctx = $0; next }
      /^\+\+\+/  { next }
      /^\+/      { if (ctx ~ /mod tests/) next; print }
    ' \
  | grep -oE '\bpub (fn|struct|enum|trait|type|const|mod) [A-Za-z_][A-Za-z0-9_]*' \
  | awk '{ print $3 }' \
  | sort -u)

api_code="$(read_doc "$API_DOC" | code_only)"

for item in $added_pub; do
  if ! grep -qE "\b${item}\b" <<< "$api_code"; then
    emit "$API_DOC" "public item \`${item}\` is added by this change and appears in no code span in $API_DOC"
  fi
done

# 2. new module files
new_modules=$(git diff --name-status --diff-filter=A "$RANGE" -- "$SRC_PREFIX" \
  | awk '{ print $2 }' \
  | grep -E '\.rs$' \
  | xargs -r -n1 basename \
  | sed 's/\.rs$//' \
  | grep -vE '^(mod|lib|prelude)$' \
  | sort -u)

arch_text="$(read_doc "$ARCH_DOC")"
layer_text="$(read_doc "$LAYER_DOC")"

for m in $new_modules; do
  grep -qE "\b${m}\b" <<< "$arch_text" \
    || emit "$ARCH_DOC" "new module \`${m}\` has no section in $ARCH_DOC"
  grep -qE "\b${m}\b" <<< "$layer_text" \
    || emit "$LAYER_DOC" "new module \`${m}\` is missing from the layer table in $LAYER_DOC"
done

echo
if [ "$findings" -eq 0 ]; then
  echo "docs-surface: no undocumented public surface in $RANGE"
  echo "docs-surface: clean over \`$RANGE\`" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
else
  echo "docs-surface: $findings finding(s) over $RANGE. This is a warning, not a failure."
  echo "" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  echo "_$findings finding(s). Warning only: nothing here blocks a merge._" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
fi

exit 0

#!/usr/bin/env bash
# Fail-closed structural policy for Braid's GitHub Actions workflow.
#
# This is intentionally a small source-level policy check, not a second CI
# engine. GitHub remains authoritative for workflow semantics; this script
# rejects the local failure shapes that have already produced false confidence
# in Braid: mutable action refs, swallowed shell failures, conditional scope
# skips, unbounded jobs, and cleanup that can run before producers finish.

set -euo pipefail

MAX_WORKFLOW_BYTES=524288

fail() {
  echo "ci-policy: $*" >&2
  exit 1
}

job_ids() {
  awk '
    $0 == "jobs:" { in_jobs = 1; next }
    in_jobs && /^[^ ]/ { exit }
    in_jobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
      line = $0
      sub(/^  /, "", line)
      sub(/:[[:space:]]*$/, "", line)
      print line
    }
  ' "$1"
}

missing_timeout_jobs() {
  awk '
    function finish_job() {
      if (job != "" && !has_timeout) print job
    }
    $0 == "jobs:" { in_jobs = 1; next }
    in_jobs && /^[^ ]/ { finish_job(); job = ""; exit }
    in_jobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
      finish_job()
      job = $0
      sub(/^  /, "", job)
      sub(/:[[:space:]]*$/, "", job)
      has_timeout = 0
      next
    }
    in_jobs && job != "" && /^    timeout-minutes:[[:space:]]*[1-9][0-9]*[[:space:]]*$/ {
      has_timeout = 1
    }
    END { finish_job() }
  ' "$1"
}

cleanup_value() {
  local workflow="$1"
  local key="$2"
  awk -v key="$key" '
    $0 == "  cleanup:" { in_cleanup = 1; next }
    in_cleanup && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { exit }
    in_cleanup && index($0, "    " key ":") == 1 {
      line = $0
      sub("^    " key ":[[:space:]]*", "", line)
      print line
      exit
    }
  ' "$workflow"
}

check_workflow() {
  local workflow="$1"
  local bytes action_count missing cleanup_needs cleanup_if normalized_needs job shell_or

  [[ -f "$workflow" ]] || fail "workflow not found: $workflow"

  bytes="$(wc -c < "$workflow" | tr -d '[:space:]')"
  [[ "$bytes" =~ ^[0-9]+$ ]] || fail "could not measure workflow bytes"
  (( bytes <= MAX_WORKFLOW_BYTES )) ||
    fail "workflow is $bytes bytes; ceiling is $MAX_WORKFLOW_BYTES"

  action_count=0
  while IFS= read -r action_ref; do
    [[ -n "$action_ref" ]] || continue
    action_count=$((action_count + 1))
    if [[ "$action_ref" == ./* || "$action_ref" == docker://* ]]; then
      continue
    fi
    if [[ ! "$action_ref" =~ ^[^[:space:]@]+/[^[:space:]@]+@[0-9a-f]{40}([[:space:]]+#.*)?$ ]]; then
      fail "external action is not pinned to a full commit SHA: $action_ref"
    fi
  done < <(
    awk '
      /^[[:space:]]*-[[:space:]]+uses:[[:space:]]*/ {
        line = $0
        sub(/^[[:space:]]*-[[:space:]]+uses:[[:space:]]*/, "", line)
        print line
      }
    ' "$workflow"
  )
  (( action_count > 0 )) || fail "workflow contains no action references"

  if grep -En '^[[:space:]]*continue-on-error:[[:space:]]*true([[:space:]]*(#.*)?)?$' "$workflow"; then
    fail "continue-on-error true makes a required step fail open"
  fi
  shell_or='||'
  if awk -v needle="$shell_or true" '
    /^[[:space:]]*#/ { next }
    index($0, needle) { print NR ":" $0; found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$workflow"; then
    fail "shell failure is swallowed with $shell_or true"
  fi
  if grep -En '(needs\.scope|code_changed|docs-only)' "$workflow"; then
    fail "scope-based skips are forbidden; every change runs the full gate"
  fi

  missing="$(missing_timeout_jobs "$workflow")"
  [[ -z "$missing" ]] || fail "jobs missing a positive timeout: ${missing//$'\n'/, }"

  cleanup_needs="$(cleanup_value "$workflow" needs)"
  [[ "$cleanup_needs" =~ ^\[.*\]$ ]] ||
    fail "cleanup needs must be an explicit inline list"
  normalized_needs="$(printf '%s' "$cleanup_needs" | sed -E 's/^\[//; s/\]$//; s/[[:space:]]//g')"
  while IFS= read -r job; do
    [[ "$job" == cleanup ]] && continue
    case ",$normalized_needs," in
      *",$job,"*) ;;
      *) fail "cleanup does not wait for job: $job" ;;
    esac
  done < <(job_ids "$workflow")

  cleanup_if="$(cleanup_value "$workflow" if)"
  [[ "$cleanup_if" == "always()" || "$cleanup_if" == '${{ always() }}' ]] ||
    fail "cleanup must run with if: always()"

  echo "ci-policy: OK actions=$action_count jobs=$(job_ids "$workflow" | wc -l | tr -d '[:space:]') bytes=$bytes"
}

expect_reject() {
  local name="$1"
  local fixture="$2"
  if (check_workflow "$fixture") >/dev/null 2>&1; then
    fail "negative fixture was accepted: $name"
  fi
  echo "ci-policy: rejected $name"
}

self_test() {
  local workflow="$1"
  local work good fixture shell_or

  check_workflow "$workflow"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' RETURN
  good="$work/good.yml"

  printf '%s\n' \
    'name: policy-fixture' \
    'jobs:' \
    '  build:' \
    '    runs-on: self-hosted' \
    '    timeout-minutes: 5' \
    '    steps:' \
    '      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1' \
    '  cleanup:' \
    '    needs: [build]' \
    '    runs-on: self-hosted' \
    '    timeout-minutes: 5' \
    '    if: always()' \
    '    steps:' \
    '      - run: echo clean' > "$good"
  check_workflow "$good" >/dev/null

  fixture="$work/mutable-action.yml"
  sed 's/@3d3c42e5aac5ba805825da76410c181273ba90b1/@v7/' "$good" > "$fixture"
  expect_reject mutable-action "$fixture"

  fixture="$work/continue-on-error.yml"
  awk '{ print; if ($0 ~ /- run: echo clean/) print "        continue-on-error: true" }' \
    "$good" > "$fixture"
  expect_reject continue-on-error "$fixture"

  fixture="$work/swallowed-shell.yml"
  cp "$good" "$fixture"
  shell_or='||'
  printf '      - run: false %s true\n' "$shell_or" >> "$fixture"
  expect_reject swallowed-shell "$fixture"

  fixture="$work/missing-timeout.yml"
  awk '!removed && /timeout-minutes:/ { removed = 1; next } { print }' "$good" > "$fixture"
  expect_reject missing-timeout "$fixture"

  fixture="$work/early-cleanup.yml"
  sed 's/needs: \[build\]/needs: []/' "$good" > "$fixture"
  expect_reject early-cleanup "$fixture"

  fixture="$work/scope-skip.yml"
  cp "$good" "$fixture"
  printf '%s\n' '    if: needs.scope.outputs.code_changed == '\''yes'\''' >> "$fixture"
  expect_reject scope-skip "$fixture"

  echo "ci-policy: self-test OK (6 negative fixtures)"
}

if [[ "${1:-}" == "--self-test" ]]; then
  [[ $# -eq 2 ]] || fail "usage: $0 --self-test <workflow>"
  self_test "$2"
else
  [[ $# -eq 1 ]] || fail "usage: $0 <workflow>"
  check_workflow "$1"
fi

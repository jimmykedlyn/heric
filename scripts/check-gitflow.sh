#!/usr/bin/env bash
# Validates the source and target branch pair for a pull request.
# Usage: check-gitflow.sh <base-branch> <head-branch>
set -euo pipefail

base="${1:-}"
head="${2:-}"

fail() {
  echo "::error::$1"
  exit 1
}

case "$head" in
  feature/*)
    [ "$base" = "develop" ] || fail "feature branches must target develop (got '$base')"
    ;;
  dependabot/*)
    # Dependabot is an automated contributor, not a Gitflow actor: its
    # branches are opened against the default branch and never merged by hand
    # into anything else.
    [ "$base" = "develop" ] || fail "dependabot branches must target develop (got '$base')"
    ;;
  release/*)
    [ "$base" = "main" ] || fail "release branches must target main (got '$base')"
    ;;
  hotfix/*)
    [ "$base" = "main" ] || fail "hotfix branches must target main (got '$base')"
    ;;
  main)
    [ "$base" = "develop" ] || fail "main may only target develop, and only for a back-merge (got '$base')"
    ;;
  *)
    fail "branch '$head' does not follow the Gitflow naming (feature/*, dependabot/*, release/*, hotfix/*)"
    ;;
esac

echo "Branch pair '$head -> $base' is valid."

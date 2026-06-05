#!/usr/bin/env bash
# SPDX-License-Identifier: MIT (script itself MIT-licensed; the Studio
# source it guards is proprietary).
# Copyright (c) 2026 Gourav Shah, Vivian Aranha
#
# Fails (exit 1) if the current diff touches BOTH OSS-side paths and
# `pro/` paths in the same commit. Prevents accidental leakage of
# proprietary code into the public-facing OSS surface, and prevents
# commits that would be confusing to cherry-pick OSS -> upstream.
#
# Usage:
#   bash scripts/check-no-pro-leak.sh              # against HEAD~1..HEAD
#   bash scripts/check-no-pro-leak.sh --staged     # against `git diff --cached`
#
# Exit codes:
#   0  — clean (diff touches only OSS, or only pro/, or nothing)
#   1  — leak detected (diff touches both)
#   2  — usage/internal error

set -euo pipefail

MODE="${1:-default}"

if [[ "$MODE" == "--staged" ]]; then
  CHANGED_FILES=$(git diff --cached --name-only || true)
else
  # Default: compare against the previous commit. In CI this is
  # HEAD~1..HEAD; locally this is the most recent commit.
  CHANGED_FILES=$(git diff --name-only HEAD~1 HEAD 2>/dev/null || git diff --cached --name-only)
fi

if [[ -z "$CHANGED_FILES" ]]; then
  echo "OK: no file changes detected."
  exit 0
fi

# PRO-side paths: anything under pro/, the proprietary LICENSE-STUDIO,
# Studio-only docs under docs/studio-*, and the build.config.ts overlay.
PRO_PATTERN='^pro/|^LICENSE-STUDIO$|^docs/studio-|^build\.config\.ts$'

# OSS-side: everything else (we use cv-style exclude).
PRO_TOUCHED=$(echo "$CHANGED_FILES" | grep -cE "$PRO_PATTERN" || true)
OSS_TOUCHED=$(echo "$CHANGED_FILES" | grep -cvE "$PRO_PATTERN" || true)

if [[ "$OSS_TOUCHED" -gt 0 && "$PRO_TOUCHED" -gt 0 ]]; then
  echo "ERROR: Commit touches BOTH OSS-side files and pro/ side files."
  echo ""
  echo "OSS-side files in this diff:"
  echo "$CHANGED_FILES" | grep -vE "$PRO_PATTERN" | sed 's/^/  /'
  echo ""
  echo "Pro-side files in this diff:"
  echo "$CHANGED_FILES" | grep -E "$PRO_PATTERN" | sed 's/^/  /'
  echo ""
  echo "Split into two commits: one for upstream OSS sync, one for pro/ changes."
  echo "Run 'git reset HEAD~1' to unstage, then commit each side separately."
  exit 1
fi

if [[ "$PRO_TOUCHED" -gt 0 ]]; then
  echo "OK: pro/-only changes ($PRO_TOUCHED file(s))."
else
  echo "OK: OSS-only changes ($OSS_TOUCHED file(s))."
fi
exit 0

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Gourav Shah, Vivian Aranha
#
# Self-test for check-no-pro-leak.sh. Uses a throwaway git repo in
# /tmp so we don't pollute the real working tree. Asserts:
# 1. Commit touching only OSS files     => exit 0
# 2. Commit touching only pro/ files    => exit 0
# 3. Commit touching BOTH               => exit 1
# 4. Commit touching only docs/studio-* and LICENSE-STUDIO => exit 0 (treated as pro)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GUARDRAIL="$SCRIPT_DIR/check-no-pro-leak.sh"

if [[ ! -x "$GUARDRAIL" ]]; then
  echo "FAIL: guardrail script not executable at $GUARDRAIL"
  exit 1
fi

TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT
cd "$TMP"
git init -q
git config user.email "test@learnforge.dev"
git config user.name "test"

# Bootstrap with an unrelated file so HEAD~1 exists.
mkdir -p src
echo "bootstrap" > README.md
git add README.md && git commit -q -m "bootstrap"

# Bring the guardrail into the fixture repo for local sourcing.
mkdir -p scripts
cp "$GUARDRAIL" scripts/check-no-pro-leak.sh
chmod +x scripts/check-no-pro-leak.sh
git add scripts/check-no-pro-leak.sh && git commit -q -m "add guardrail"

# Test 1: OSS-only commit -> exit 0.
echo "alpha" > src/oss-feature.ts
git add src/oss-feature.ts && git commit -q -m "feat: oss-only"
if ! bash scripts/check-no-pro-leak.sh > /dev/null; then
  echo "FAIL test 1: guardrail rejected an OSS-only commit"; exit 1
fi
echo "PASS test 1: OSS-only commit accepted"

# Test 2: pro/-only commit -> exit 0.
mkdir -p pro/src-tauri-pro
echo "beta" > pro/src-tauri-pro/something.rs
git add pro/src-tauri-pro/something.rs && git commit -q -m "feat: pro-only"
if ! bash scripts/check-no-pro-leak.sh > /dev/null; then
  echo "FAIL test 2: guardrail rejected a pro/-only commit"; exit 1
fi
echo "PASS test 2: pro/-only commit accepted"

# Test 3: mixed commit -> exit 1.
echo "gamma-oss" > src/oss-feature.ts
echo "gamma-pro" > pro/src-tauri-pro/something.rs
git add src/oss-feature.ts pro/src-tauri-pro/something.rs && git commit -q -m "BAD: mixed"
if bash scripts/check-no-pro-leak.sh > /dev/null 2>&1; then
  echo "FAIL test 3: guardrail accepted a mixed commit"; exit 1
fi
echo "PASS test 3: mixed commit rejected"

# Test 4: docs/studio-* + LICENSE-STUDIO (pro-side per pattern) -> exit 0.
git reset --hard HEAD~1 -q
mkdir -p docs
echo "tier-info" > docs/studio-landing-outline.md
echo "proprietary" > LICENSE-STUDIO
git add docs/studio-landing-outline.md LICENSE-STUDIO && git commit -q -m "studio docs"
if ! bash scripts/check-no-pro-leak.sh > /dev/null; then
  echo "FAIL test 4: guardrail rejected studio-docs commit"; exit 1
fi
echo "PASS test 4: studio-docs commit accepted (pro-side)"

echo ""
echo "ALL TESTS PASSED"

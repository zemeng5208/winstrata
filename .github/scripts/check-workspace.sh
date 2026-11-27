#!/usr/bin/env bash
set -euo pipefail

git diff --check HEAD^1 HEAD
if [[ -n "$(git ls-files --cached --ignored --exclude-standard)" ]]; then
  printf '%s\n' 'Generated or ignored files are tracked.' >&2
  git ls-files --cached --ignored --exclude-standard
  exit 1
fi
node --test --test-isolation=none .github/scripts/merge-policy.test.mjs
cargo dev init
cargo dev build
cargo dev verify
git diff --exit-code -- Cargo.lock pnpm-lock.yaml

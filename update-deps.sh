#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
node "${repo_root}/v2/cargo-fui/scripts/update-upstream-versions.mjs"

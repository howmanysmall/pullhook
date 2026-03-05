#!/usr/bin/env bash

set -euo pipefail

bun x --bun lint-staged
gitleaks detect --source .
cargo clippy --all-targets -- -D warnings
cargo audit --quiet
cargo deny check
cargo shear

exit 0

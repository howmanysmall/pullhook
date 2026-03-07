#!/usr/bin/env bash

readonly STYLUA2=$(which stylua2)
readonly TARGET_DIRECTORY=$(dirname "${STYLUA2}")

cargo build --release

xcp target/release/pullhook "${TARGET_DIRECTORY}/pullhook-latest" --no-progress
echo "Built pullhook and copied to ${TARGET_DIRECTORY}/pullhook-latest"

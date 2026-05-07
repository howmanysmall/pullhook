#!/usr/bin/env bash

readonly DEFAULT_PATH="."

flags=() paths=()
for a; do [[ $a == -* ]] && flags+=("$a") || paths+=("$a"); done
[[ ${#paths[@]} -eq 0 ]] && paths=("$DEFAULT_PATH")
echo "${flags[*]} ${paths[*]}"

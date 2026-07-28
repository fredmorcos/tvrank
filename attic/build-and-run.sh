#!/bin/bash

GIT_HASH=$(git rev-parse --short HEAD)

for _i in {0..2}; do
  cargo run --release -- search "pirates of the caribbean" 2>&1 | grep -E "^Total time:" | sed -e "s/^Total time:/$GIT_HASH/" >> "commit-results.txt"
  if [[ $? != 0 ]]; then
    exit 0
  fi
done

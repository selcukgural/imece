#!/bin/sh
# imece runs this before `:tidy` removes a worktree, once you said yes (record 96).
set -eu
echo "torn down: $PWD" >> "${TMPDIR:-/tmp}/shelf-teardown.log"

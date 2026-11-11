#!/bin/bash
# usage: vm-test.sh <cargo test args...>
set -e
source "$(dirname "$0")/vm-common.sh"
sync_tree
ssh_quiet "cd src\\winuake && cargo xtask gen-bindings >nul 2>&1 && cargo test $*"

#!/bin/bash
# usage: vm-test-interactive.sh <cargo test args...>
# Builds over SSH, then runs the tests in the logged-in console session via a scheduled task (DirectComposition needs a desktop).
set -e
source "$(dirname "$0")/vm-common.sh"
sync_tree
scp -q "${SSH_OPTIONS[@]}" "$REPO_ROOT/scripts/run-interactive-test.cmd" "$VM_SSH:run-interactive-test.cmd" 2>/dev/null
ssh_quiet "cd src\\winuake && cargo xtask gen-bindings >nul 2>&1 && cargo test --no-run $*"
ssh_quiet "del %USERPROFILE%\\interactive-test.log 2>nul & schtasks /create /f /tn winuake-itest /sc once /st 23:59 /it /rl limited /tr \"%USERPROFILE%\\run-interactive-test.cmd $*\" >nul & schtasks /run /tn winuake-itest >nul"
for _ in $(seq 1 150); do
  log="$(ssh_quiet 'type %USERPROFILE%\interactive-test.log 2>nul')"
  if grep -q '^DONE' <<<"$log"; then grep -v '^DONE' <<<"$log"; exit 0; fi
  sleep 2
done
echo "timed out waiting for tests" >&2
echo "$log"
exit 1

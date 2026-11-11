#!/bin/bash
# usage: vm-run-app.sh   -- sync, build, (re)start the app in the console session, print its log after a few seconds
set -e
source "$(dirname "$0")/vm-common.sh"
sync_tree
scp -q "${SSH_OPTIONS[@]}" "$REPO_ROOT/scripts/run-app.cmd" "$VM_SSH:run-app.cmd" 2>/dev/null
ssh_quiet 'taskkill /f /im winuake.exe >nul 2>&1 & cd src\winuake && cargo xtask gen-bindings >nul 2>&1 && cargo build -p winuake 2>&1 | findstr /R /C:"^error" /C:"^warning" /C:"Finished"'
ssh_quiet 'del %USERPROFILE%\winuake-app.log 2>nul & schtasks /create /f /tn winuake-app /sc once /st 23:59 /it /rl limited /tr "%USERPROFILE%\run-app.cmd" >nul & schtasks /run /tn winuake-app >nul & ping -n 8 127.0.0.1 >nul & type %USERPROFILE%\winuake-app.log'

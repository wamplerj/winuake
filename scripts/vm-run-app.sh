#!/bin/bash
# usage: [WINUAKE_WINDOW_SETTINGS="frames=50+keep_open=false"] vm-run-app.sh   -- sync, build, (re)start the app in the console session, print its log after a few seconds
set -e
source "$(dirname "$0")/vm-common.sh"
sync_tree
scp -q "${SSH_OPTIONS[@]}" "$REPO_ROOT/scripts/run-app.cmd" "$VM_SSH:run-app.cmd" 2>/dev/null
ssh_quiet 'taskkill /f /im winuake.exe >nul 2>&1 & for /L %i in (1,1,20) do @(tasklist /fi "imagename eq winuake.exe" | find /i "winuake.exe" >nul && (ping -n 2 127.0.0.1 >nul) || (exit /b 0))'
# A hung old instance can keep the exe locked; a running exe can still be renamed, which frees the name for the new build.
ssh_quiet 'cd src\winuake\target\debug 2>nul && (if exist winuake.exe ren winuake.exe winuake.stale-%RANDOM%.exe) & del winuake.stale-*.exe 2>nul'
ssh_quiet 'cd src\winuake && cargo xtask gen-bindings >nul 2>&1 && cargo build -p winuake 2>&1 | findstr /R /C:"^error" /C:"^warning" /C:"Finished" /C:"denied" /C:"-->"'
# A unique task name per run: a stuck previous instance keeps its task "running" and would block a rerun of the same task.
task="winuake-run-$(date +%s)"
ssh_quiet "del %USERPROFILE%\\winuake-app.last 2>nul & schtasks /create /f /tn $task /sc once /st 23:59 /it /rl limited /tr \"%USERPROFILE%\\run-app.cmd ${WINUAKE_WINDOW_SETTINGS:-}\" >nul & schtasks /run /tn $task >nul & ping -n 8 127.0.0.1 >nul & for /f \"usebackq delims=\" %f in (\"%USERPROFILE%\\winuake-app.last\") do @type \"%f\""
ssh_quiet "schtasks /delete /f /tn $task >nul 2>&1"

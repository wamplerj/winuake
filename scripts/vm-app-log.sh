#!/bin/bash
# usage: vm-app-log.sh   -- print the log of the most recent app run (ANSI colors stripped)
source "$(dirname "$0")/vm-common.sh"
ssh_quiet 'for /f "usebackq delims=" %f in ("%USERPROFILE%\winuake-app.last") do @type "%f"' | sed 's/\x1b\[[0-9;]*m//g'

@echo off
cd /d %USERPROFILE%\src\winuake\target\debug
winuake.exe > %USERPROFILE%\winuake-app.log 2>&1
echo exit=%errorlevel% >> %USERPROFILE%\winuake-app.log

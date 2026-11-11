@echo off
if not "%~1"=="" set "WINUAKE_WINDOW_SETTINGS=%*"
if exist "%USERPROFILE%\allow-multiple.flag" set "WINUAKE_ALLOW_MULTIPLE=1"
set "LOG=%USERPROFILE%\winuake-app-%RANDOM%.log"
>"%USERPROFILE%\winuake-app.last" echo %LOG%
cd /d %USERPROFILE%\src\winuake\target\debug
winuake.exe > "%LOG%" 2>&1
echo exit=%errorlevel% >> "%LOG%"

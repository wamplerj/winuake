@echo off
cd /d %USERPROFILE%\src\winuake
cargo test %* > %USERPROFILE%\interactive-test.log 2>&1
echo exit=%errorlevel% >> %USERPROFILE%\interactive-test.log
echo DONE >> %USERPROFILE%\interactive-test.log

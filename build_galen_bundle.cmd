@echo off
cd /d "%~dp0rust\crates\galen"
set "PATH=%PATH%;C:\Users\DORAT\.cargo\bin"
set CARGO_BUILD_JOBS=2
set HTTP_PROXY=http://127.0.0.1:7890
set HTTPS_PROXY=http://127.0.0.1:7890
call npm run tauri build
echo GALEN_BUNDLE_EXIT=%ERRORLEVEL%

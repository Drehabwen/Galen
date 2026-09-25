@echo off
cd /d "%~dp0rust"
set CARGO_BUILD_JOBS=2
"C:\Users\DORAT\.cargo\bin\cargo.exe" build --release -p galen -j 2
echo GALEN_BUILD_EXIT=%ERRORLEVEL%

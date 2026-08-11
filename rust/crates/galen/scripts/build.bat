@echo off
set "PATH=C:\Users\DORAT\.cargo\bin;C:\Program Files\nodejs;%PATH%"
cd /d D:\DEV\toolchains\claw-code\rust\crates\galen\src-tauri
cargo --version
echo ---- BUILDING ----
..\node_modules\.bin\tauri.cmd build
echo ---- DONE ----
pause

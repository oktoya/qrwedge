@echo off
rem Build single Windows exe -> dist\QRWedge.exe
rem Needs Rust (stable-x86_64-pc-windows-gnullvm) and llvm-mingw. Defaults match the C:\rust install.
cd /d "%~dp0"
if not defined RUSTUP_HOME set RUSTUP_HOME=C:\rust\rustup
if not defined CARGO_HOME set CARGO_HOME=C:\rust\cargo
set PATH=%CARGO_HOME%\bin;%PATH%;C:\rust\mingw\bin
cargo build --release --bin QRWedge || exit /b 1
if not exist dist mkdir dist
copy /y target\release\QRWedge.exe dist\QRWedge.exe >nul
echo Done: dist\QRWedge.exe

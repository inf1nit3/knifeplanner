@echo off
REM ==========================================================================
REM  KnifePlanner - Windows-Build
REM  Baut knifeplanner.exe und legt es in dist\ ab.
REM  Voraussetzung: Rust (https://rustup.rs) installiert.
REM ==========================================================================
setlocal

cd /d "%~dp0"

echo.
echo === KnifePlanner - Windows-Build ===
echo.

where cargo >nul 2>nul
if errorlevel 1 (
    echo [FEHLER] Rust ist nicht installiert.
    echo Bitte rustup-init.exe von https://rustup.rs ausfuehren und danach erneut starten.
    echo.
    pause
    exit /b 1
)

echo [1/3] Rust-Version ...
cargo --version
echo.

echo [2/3] Baue Release-Version ...
cargo build --release
if errorlevel 1 (
    echo.
    echo [FEHLER] Build fehlgeschlagen.
    pause
    exit /b 1
)
echo.

echo [3/3] Kopiere nach dist\ ...
if not exist "dist" mkdir "dist"
copy /Y "target\release\knifeplanner.exe" "dist\knifeplanner.exe" >nul
copy /Y "README.md" "dist\" >nul 2>nul
echo Fertig: dist\knifeplanner.exe
echo.
echo Start: dist\knifeplanner.exe
echo API-Schluessel koennst du in der App unter "Einstellungen / API" eintragen.
echo.

pause

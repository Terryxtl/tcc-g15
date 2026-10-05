@echo off
setlocal

cd /d "%~dp0" || goto :error

echo Compiling release build...
set "RUSTFLAGS=%RUSTFLAGS% -C target-feature=+crt-static"
cargo build --locked --release --target x86_64-pc-windows-msvc --bin tcc-g15 || goto :error

echo Preparing dist\tcc-g15\ ...
if not exist dist\tcc-g15\icons mkdir dist\tcc-g15\icons
if errorlevel 1 goto :error
copy /Y target\x86_64-pc-windows-msvc\release\tcc-g15.exe dist\tcc-g15\tcc-g15.exe || goto :error
copy /Y icons\gaugeIcon.ico dist\tcc-g15\icons\gaugeIcon.ico || goto :error

echo Compiling installer...
set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%~dp0build\tools\inno-setup-6.7.3\ISCC.exe"
if not exist "%ISCC%" set "ISCC=iscc.exe"
"%ISCC%" installer-inno-config.iss || goto :error

echo.
echo Installer written to dist\
exit /b 0
:error
echo Failed with error #%errorlevel%.
exit /b %errorlevel%

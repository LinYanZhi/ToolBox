@echo off
call mm home

call cargo build -p ls --release
call cargo build -p as --release

copy target\release\as.exe as.exe
copy target\release\ls.exe ls.exe

set TOOLS_DIR=aminos-source\tools

if not exist "%TOOLS_DIR%\ls" mkdir "%TOOLS_DIR%\ls"
if not exist "%TOOLS_DIR%\as" mkdir "%TOOLS_DIR%\as"

powershell -Command "Compress-Archive -Path target\release\ls.exe -DestinationPath '%TOOLS_DIR%\ls\ls.zip' -Force"
echo ✓ ls.zip

powershell -Command "Compress-Archive -Path target\release\as.exe -DestinationPath '%TOOLS_DIR%\as\as.zip' -Force"
echo ✓ as.zip

echo.
echo ok!

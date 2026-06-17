@echo off

del as.exe
del e.exe

del %USERPROFILE%\Desktop\as.exe

call cargo build -p as --release
call cargo build -p e --release

copy target\release\as.exe as.exe
copy target\release\as.exe %USERPROFILE%\Desktop
copy target\release\e.exe e.exe

set TOOLS_DIR=aminos-source\tools

if not exist "%TOOLS_DIR%\as" mkdir "%TOOLS_DIR%\as"
if not exist "%TOOLS_DIR%\e" mkdir "%TOOLS_DIR%\e"

powershell -Command "Compress-Archive -Path target\release\as.exe -DestinationPath '%TOOLS_DIR%\as\as.zip' -Force"
echo [OK] as.zip

powershell -Command "Compress-Archive -Path target\release\e.exe -DestinationPath '%TOOLS_DIR%\e\e.zip' -Force"
echo [OK] e.zip

echo.
echo ok!

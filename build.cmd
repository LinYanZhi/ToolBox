@echo off

del as.exe
del ls.exe
del pp.exe
del ss.exe

del %USERPROFILE%\Desktop\as.exe

call cargo build -p as --release
call cargo build -p ls --release
call cargo build -p pp --release
call cargo build -p ss --release

copy target\release\as.exe as.exe
copy target\release\as.exe %USERPROFILE%\Desktop
copy target\release\ls.exe ls.exe
copy target\release\pp.exe pp.exe
copy target\release\ss.exe ss.exe

set TOOLS_DIR=aminos-source\tools

if not exist "%TOOLS_DIR%\as" mkdir "%TOOLS_DIR%\as"
if not exist "%TOOLS_DIR%\ls" mkdir "%TOOLS_DIR%\ls"
if not exist "%TOOLS_DIR%\pp" mkdir "%TOOLS_DIR%\pp"
if not exist "%TOOLS_DIR%\ss" mkdir "%TOOLS_DIR%\ss"

powershell -Command "Compress-Archive -Path target\release\as.exe -DestinationPath '%TOOLS_DIR%\as\as.zip' -Force"
echo [OK] as.zip

powershell -Command "Compress-Archive -Path target\release\ls.exe -DestinationPath '%TOOLS_DIR%\ls\ls.zip' -Force"
echo [OK] ls.zip

powershell -Command "Compress-Archive -Path target\release\pp.exe -DestinationPath '%TOOLS_DIR%\pp\pp.zip' -Force"
echo [OK] pp.zip

powershell -Command "Compress-Archive -Path target\release\ss.exe -DestinationPath '%TOOLS_DIR%\ss\ss.zip' -Force"
echo [OK] ss.zip

echo.
echo ok!

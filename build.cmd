@echo off
setlocal enabledelayedexpansion

echo == Clean old files ==
del as.exe 2>nul

echo == Build as ==
cd aspkg
call cargo build --release
if %errorlevel% neq 0 exit /b %errorlevel%
copy target\release\as.exe ..\as.exe >nul
cd ..
echo [OK] as.exe

echo.
echo ================ build OK ================
echo   as.exe
echo ==========================================
echo.

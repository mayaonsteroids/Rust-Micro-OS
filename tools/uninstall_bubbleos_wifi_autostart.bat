@echo off
setlocal enabledelayedexpansion

title BubbleOS Wi-Fi Helper - Autostart Removal
echo =================================================================
echo       BUBBLEOS WI-FI HELPER: AUTOSTART UNINSTALLER
echo =================================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "PID_FILE=%SCRIPT_DIR%wifi_helper.pid"

REM 1. Remove HKCU Run key if present
echo [*] Removing registry autostart entry...
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "BubbleOS_WiFi_Helper" /f >nul 2>&1
echo [+] Registry entry removed.

REM 2. Remove Startup folder VBS launcher
set "STARTUP_FOLDER=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup"
set "VBS_FILE=%STARTUP_FOLDER%\BubbleOS_WiFi_Helper.vbs"

if exist "%VBS_FILE%" (
    echo [*] Removing Startup folder launcher...
    del /f /q "%VBS_FILE%" >nul 2>&1
    echo [+] Startup launcher deleted.
)

REM 3. Terminate running helper process
if exist "%PID_FILE%" (
    set /p HELPER_PID=<"%PID_FILE%"
    if defined HELPER_PID (
        echo [*] Stopping running helper process PID: !HELPER_PID!
        taskkill /F /PID !HELPER_PID! >nul 2>&1
    )
    del /f /q "%PID_FILE%" >nul 2>&1
)

REM 4. Invoke helper with --stop for clean shutdown
where pythonw.exe >nul 2>&1
if !errorlevel! equ 0 (
    pythonw.exe "%SCRIPT_DIR%windows_host_wifi_helper.py" --stop >nul 2>&1
) else (
    python.exe "%SCRIPT_DIR%windows_host_wifi_helper.py" --stop >nul 2>&1
)

echo.
echo =================================================================
echo   [SUCCESS] BubbleOS Wi-Fi Helper autostart has been removed.
echo =================================================================
echo.

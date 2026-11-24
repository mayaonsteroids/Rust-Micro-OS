@echo off
setlocal enabledelayedexpansion

title BubbleOS Wi-Fi Helper - Autostart Removal
echo =================================================================
echo       BUBBLEOS WI-FI HELPER: AUTOSTART UNINSTALLER
echo =================================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "PID_FILE=%SCRIPT_DIR%wifi_helper.pid"

:: 1. Remove HKCU Run key
echo [*] Removing registry autostart entry...
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "BubbleOS_WiFi_Helper" /f >nul 2>&1
echo [+] Registry entry removed.

:: 2. Remove Startup folder VBS launcher
set "STARTUP_FOLDER=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup"
set "VBS_FILE=%STARTUP_FOLDER%\BubbleOS_WiFi_Helper.vbs"

if exist "%VBS_FILE%" (
    echo [*] Removing Startup folder launcher...
    del /f /q "%VBS_FILE%" >nul 2>&1
    echo [+] Startup launcher deleted.
)

:: 3. Terminate running helper process
if exist "%PID_FILE%" (
    set /p HELPER_PID=<"%PID_FILE%"
    if defined HELPER_PID (
        echo [*] Stopping running helper process (PID: !HELPER_PID!)...
        taskkill /F /PID !HELPER_PID! >nul 2>&1
    )
    del /f /q "%PID_FILE%" >nul 2>&1
)

:: Also kill any python process running windows_host_wifi_helper.py
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*windows_host_wifi_helper.py*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue; Write-Host '[+] Terminated helper process PID:' $_.ProcessId }"

echo.
echo =================================================================
echo   [SUCCESS] BubbleOS Wi-Fi Helper autostart has been removed.
echo =================================================================
echo.

@echo off
setlocal enabledelayedexpansion

title BubbleOS Wi-Fi Helper - Autostart Setup
echo =================================================================
echo        BUBBLEOS WI-FI HELPER: AUTOSTART INSTALLER
echo =================================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "HELPER_SCRIPT=%SCRIPT_DIR%windows_host_wifi_helper.py"
set "PID_FILE=%SCRIPT_DIR%wifi_helper.pid"

if not exist "%HELPER_SCRIPT%" (
    echo [-] Error: windows_host_wifi_helper.py not found at:
    echo     "%HELPER_SCRIPT%"
    pause
    exit /b 1
)

REM Find pythonw.exe
set "PYTHONW="
where pythonw.exe >nul 2>&1
if !errorlevel! equ 0 (
    for /f "delims=" %%I in ('where pythonw.exe') do (
        if not defined PYTHONW set "PYTHONW=%%I"
    )
)

if not defined PYTHONW (
    if exist "%LOCALAPPDATA%\Microsoft\WindowsApps\pythonw.exe" (
        set "PYTHONW=%LOCALAPPDATA%\Microsoft\WindowsApps\pythonw.exe"
    )
)

if not defined PYTHONW (
    where python.exe >nul 2>&1
    if !errorlevel! equ 0 (
        for /f "delims=" %%I in ('where python.exe') do (
            set "PYDIR=%%~dpI"
            if exist "!PYDIR!pythonw.exe" set "PYTHONW=!PYDIR!pythonw.exe"
        )
    )
)

if not defined PYTHONW (
    echo [-] Warning: pythonw.exe not found on PATH. Falling back to python.exe.
    where python.exe >nul 2>&1
    if !errorlevel! equ 0 (
        for /f "delims=" %%I in ('where python.exe') do (
            if not defined PYTHONW set "PYTHONW=%%I"
        )
    )
)

if not defined PYTHONW (
    echo [-] Fatal: Python is not installed or not found on PATH.
    echo     Please install Python 3 on Windows from python.org or the Microsoft Store.
    pause
    exit /b 1
)

echo [*] Detected Python Runner : "%PYTHONW%"
echo [*] Target Helper Script   : "%HELPER_SCRIPT%"
echo.

REM 1. Remove any legacy or duplicate HKCU Run registry entry so there is only ONE autostart mechanism
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "BubbleOS_WiFi_Helper" /f >nul 2>&1

REM 2. Configure a single, 100% hidden VBScript launcher in the user's Startup folder
set "STARTUP_FOLDER=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup"
set "VBS_FILE=%STARTUP_FOLDER%\BubbleOS_WiFi_Helper.vbs"

if not exist "%STARTUP_FOLDER%" (
    echo [-] Fatal: Startup folder not found: "%STARTUP_FOLDER%"
    pause
    exit /b 1
)

echo [*] Installing silent hidden VBScript launcher in:
echo     "%VBS_FILE%"
(
    echo Set WshShell = CreateObject^("WScript.Shell"^)
    echo WshShell.Run """" ^& "%PYTHONW%" ^& """ """ ^& "%HELPER_SCRIPT%" ^& """", 0, False
) > "%VBS_FILE%"
echo [+] Startup launcher installed successfully.

REM 3. Terminate any previous helper instance to ensure fresh code runs
if exist "%PID_FILE%" (
    set /p OLD_PID=<"%PID_FILE%"
    if defined OLD_PID taskkill /F /PID !OLD_PID! >nul 2>&1
    del /f /q "%PID_FILE%" >nul 2>&1
)
"%PYTHONW%" "%HELPER_SCRIPT%" --stop >nul 2>&1

REM 4. Launch the helper immediately using wscript.exe (genuinely hidden, 0 console windows)
echo.
echo [*] Starting BubbleOS Wi-Fi Helper silently via wscript.exe (hidden window mode)...
wscript.exe "%VBS_FILE%"

REM 5. Verify helper responsiveness on port 19820
echo [*] Verifying helper endpoint at http://127.0.0.1:19820/ ...
powershell -NoProfile -Command "for ($i = 0; $i -lt 6; $i++) { Start-Sleep -Milliseconds 500; try { $res = Invoke-RestMethod -Uri 'http://127.0.0.1:19820/' -TimeoutSec 2; Write-Host '    [+] Connected to SSID:' $res.ssid; Write-Host '    [+] Signal Strength  :' ($res.signal.ToString() + '%%'); Write-Host '    [+] Host Adapter     :' $res.interface; break } catch { if ($i -eq 5) { Write-Host '    [!] Helper started (PID written to tools\wifi_helper.pid)' } } }"

echo.
echo =================================================================
echo   [SUCCESS] BubbleOS Wi-Fi Helper is permanently installed!
echo.
echo   - Runs silently in the background (no visible command window)
echo   - Launches automatically every time Windows boots or logs in
echo   - Auto-restarts on error and logs to tools\wifi_helper.log
echo   - VirtualBox can now access live Wi-Fi details at 10.0.2.2:19820
echo =================================================================
echo.

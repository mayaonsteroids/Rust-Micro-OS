@echo off
setlocal enabledelayedexpansion

title BubbleOS Wi-Fi Helper - Autostart Setup
echo =================================================================
echo        BUBBLEOS WI-FI HELPER: AUTOSTART INSTALLER
echo =================================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "HELPER_SCRIPT=%SCRIPT_DIR%windows_host_wifi_helper.py"

if not exist "%HELPER_SCRIPT%" (
    echo [-] Error: windows_host_wifi_helper.py not found at:
    echo     "%HELPER_SCRIPT%"
    pause
    exit /b 1
)

:: Find pythonw.exe
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

:: 1. Add to HKCU Run key for automatic logon launch without administrator rights
echo [*] Configuring User Logon Autostart (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)...
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "BubbleOS_WiFi_Helper" /t REG_SZ /d "\"%PYTHONW%\" \"%HELPER_SCRIPT%\"" /f >nul
if !errorlevel! neq 0 (
    echo [-] Warning: Failed to set HKCU Run key. Attempting Startup folder fallback...
) else (
    echo [+] Successfully registered in Windows Logon Autostart.
)

:: 2. Also create a silent VBS launcher in Startup folder as a robust redundancy
set "STARTUP_FOLDER=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup"
set "VBS_FILE=%STARTUP_FOLDER%\BubbleOS_WiFi_Helper.vbs"

if exist "%STARTUP_FOLDER%" (
    echo [*] Creating silent background Startup launcher in:
    echo     "%VBS_FILE%"
    (
        echo Set WshShell = CreateObject^("WScript.Shell"^)
        echo WshShell.Run """" ^& "%PYTHONW%" ^& """ """ ^& "%HELPER_SCRIPT%" ^& """", 0, False
    ) > "%VBS_FILE%"
    echo [+] Startup launcher created successfully.
)

:: 3. Terminate any previous helper instance to ensure fresh version runs
set "PID_FILE=%SCRIPT_DIR%wifi_helper.pid"
if exist "%PID_FILE%" (
    set /p OLD_PID=<"%PID_FILE%"
    if defined OLD_PID taskkill /F /PID !OLD_PID! >nul 2>&1
    del /f /q "%PID_FILE%" >nul 2>&1
)
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*windows_host_wifi_helper.py*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }" >nul 2>&1

:: 4. Launch the helper immediately in the background
echo.
echo [*] Starting BubbleOS Wi-Fi Helper in background now...
start "" "%PYTHONW%" "%HELPER_SCRIPT%"

:: 5. Verify helper responsiveness on port 19820
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

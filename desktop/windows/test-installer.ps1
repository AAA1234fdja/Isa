# CI smoke test: install, launch the real WebView, close, and uninstall.
$ErrorActionPreference = 'Stop'
$installer = Join-Path $PSScriptRoot '..\..\desktop-installer\ISA-Desktop-Prototype-Setup.exe'
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Installer is missing' }
$installDir = Join-Path $env:RUNNER_TEMP ('isa-desktop-install-' + [guid]::NewGuid().ToString('N'))
$profile = Join-Path $env:LOCALAPPDATA 'ISA\DesktopPrototype'
$sentinel = Join-Path $profile ('installer-test-' + [guid]::NewGuid().ToString('N') + '.txt')
$uninstaller = Join-Path $installDir 'unins000.exe'
$stderrFile = Join-Path $env:RUNNER_TEMP ('isa-desktop-stderr-' + [guid]::NewGuid().ToString('N') + '.txt')
New-Item -ItemType Directory -Path $profile -Force | Out-Null
Set-Content -LiteralPath $sentinel -Value 'keep user data' -NoNewline

try {
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', ('/DIR="' + $installDir + '"'))
    $setup = Start-Process -FilePath $installer -ArgumentList $arguments -PassThru -Wait
    if ($setup.ExitCode -ne 0) { throw "Installer failed with exit code $($setup.ExitCode)" }
    foreach ($name in @('Isa.Desktop.exe', 'isa.exe', 'LICENSE', 'README.md', 'unins000.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $installDir $name) -PathType Leaf)) {
            throw "Installed file missing: $name"
        }
    }
    # A missing backend must fail explicitly instead of opening an unrelated page.
    $backendPath = Join-Path $installDir 'isa.exe'
    $savedBackend = Join-Path $installDir 'isa.saved.exe'
    Move-Item -LiteralPath $backendPath -Destination $savedBackend
    try {
        $missing = Start-Process -FilePath (Join-Path $installDir 'Isa.Desktop.exe') -ArgumentList '--smoke-test' -RedirectStandardError $stderrFile -PassThru
        if (-not $missing.WaitForExit(30000)) {
            & taskkill.exe /PID $missing.Id /T /F | Out-Null
            throw 'Desktop did not report its missing backend'
        }
        $missing.Refresh()
        $details = Get-Content -LiteralPath $stderrFile -Raw
        if ($missing.ExitCode -eq 0 -or $details -notmatch 'bundled ISA backend') {
            throw "Desktop did not report its missing backend: exit $($missing.ExitCode), $details"
        }
    }
    finally { Move-Item -LiteralPath $savedBackend -Destination $backendPath }

    # The app must never attach to a different process already using its port.
    $occupied = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 4175)
    $occupied.ExclusiveAddressUse = $true
    try {
        $occupied.Start()
        $conflict = Start-Process -FilePath (Join-Path $installDir 'Isa.Desktop.exe') -ArgumentList '--smoke-test' -RedirectStandardError $stderrFile -PassThru
        if (-not $conflict.WaitForExit(30000)) {
            & taskkill.exe /PID $conflict.Id /T /F | Out-Null
            throw 'Desktop did not report its occupied port'
        }
        $conflict.Refresh()
        $details = Get-Content -LiteralPath $stderrFile -Raw
        if ($conflict.ExitCode -eq 0 -or $details -notmatch 'ISA could not bind its local port') {
            throw "Desktop did not report the port conflict: exit $($conflict.ExitCode), $details"
        }
        if ($occupied.Pending()) { throw 'Desktop connected to the unrelated listener' }
    }
    finally { $occupied.Stop() }

    # Exercise the installed executable and real WebView2 runtime. The app's
    # smoke mode uses a temporary profile and fails if the interface is missing.
    $gui = Start-Process -FilePath (Join-Path $installDir 'Isa.Desktop.exe') -ArgumentList '--smoke-test' -RedirectStandardError $stderrFile -PassThru
    if (-not $gui.WaitForExit(75000)) {
        & taskkill.exe /PID $gui.Id /T /F | Out-Null
        throw 'Desktop smoke test timed out'
    }
    $gui.Refresh()
    if ($gui.ExitCode -ne 0) {
        $details = if (Test-Path -LiteralPath $stderrFile) { Get-Content -LiteralPath $stderrFile -Raw } else { '' }
        throw "Desktop smoke test failed with exit code $($gui.ExitCode): $details"
    }
    # A clean window exit should release its own backend's fixed loopback port.
    $socket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 4175)
    $socket.ExclusiveAddressUse = $true
    try { $socket.Start() } finally { $socket.Stop() }

    $remove = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -PassThru -Wait
    if ($remove.ExitCode -ne 0) { throw "Uninstaller failed with exit code $($remove.ExitCode)" }
    if (Test-Path -LiteralPath (Join-Path $installDir 'Isa.Desktop.exe')) { throw 'Desktop executable remained after uninstall' }
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) { throw 'Uninstall removed user profile data' }
    if ((Get-Content -LiteralPath $sentinel -Raw) -ne 'keep user data') { throw 'Uninstall changed user profile data' }
    Write-Host 'Installer, WebView startup, backend shutdown and uninstaller smoke checks passed.'
}
finally {
    # The paths contain test-generated GUIDs; never remove the profile directory.
    if (Test-Path -LiteralPath $uninstaller -PathType Leaf) {
        $cleanup = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -PassThru -Wait
        if ($cleanup.ExitCode -ne 0) { Write-Warning "Cleanup uninstaller failed: $($cleanup.ExitCode)" }
    }
    Remove-Item -LiteralPath $stderrFile -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $sentinel -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $installDir) { Remove-Item -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue }
}

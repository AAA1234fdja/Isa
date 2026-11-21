#Requires -Version 5.1
<#
Read-only diagnostics for the future ISA Windows sandbox. Does not create,
start, alter or delete VMs, switches, files, network configuration or guests.
A positive result is not proof of isolation or support. Some checks require
administrator privileges and return unknown when access is denied.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

function Get-OptionalFeatureState([string] $name) {
    try {
        $feature = Get-WindowsOptionalFeature -Online -FeatureName $name -ErrorAction Stop
        return [string]$feature.State
    } catch {
        return 'Unknown'
    }
}

$report = [ordered]@{
    schemaVersion = 1
    diagnosticOnly = $true
    sandboxImplemented = $false
    platform = 'Unknown'
    windowsVersion = $null
    windowsBuild = $null
    administrator = $null
    hypervisorPresent = $null
    firmwareVirtualization = $null
    windowsSandboxFeature = 'Unknown'
    hyperVFeature = 'Unknown'
    hyperVPowerShellAvailable = $false
    existingVirtualSwitchCount = $null
    limitations = @(
        'No VM or sandbox was created or started.',
        'Feature and virtualization indicators do not verify policy, guest isolation, lifecycle, network or ISA integration.',
        'Administrator privileges may be required to query some settings; Unknown is not equivalent to Disabled.',
        'ISA sandbox execution, workspace transfer, broker and approved import remain unimplemented.'
    )
}

try {
    if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT) {
        $report.platform = 'Windows'
        $os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
        $report.windowsVersion = [string]$os.Version
        $report.windowsBuild = [string]$os.BuildNumber
    }
} catch { }
try {
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object System.Security.Principal.WindowsPrincipal($identity)
    $report.administrator = $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
} catch { }
try {
    $system = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
    if ($null -ne $system.HypervisorPresent) { $report.hypervisorPresent = [bool]$system.HypervisorPresent }
} catch { }
try {
    $processors = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop)
    if ($processors.Count -gt 0 -and @($processors | Where-Object { $null -eq $_.VirtualizationFirmwareEnabled }).Count -eq 0) {
        $report.firmwareVirtualization = @($processors | Where-Object { -not $_.VirtualizationFirmwareEnabled }).Count -eq 0
    }
} catch { }
if ($report.platform -eq 'Windows') {
    $report.windowsSandboxFeature = Get-OptionalFeatureState 'Containers-DisposableClientVM'
    $report.hyperVFeature = Get-OptionalFeatureState 'Microsoft-Hyper-V-All'
    $report.hyperVPowerShellAvailable = $null -ne (Get-Command Get-VMSwitch -ErrorAction SilentlyContinue)
    if ($report.hyperVPowerShellAvailable) {
        try { $report.existingVirtualSwitchCount = @(Get-VMSwitch -ErrorAction Stop).Count } catch { }
    }
}
$report | ConvertTo-Json -Depth 4

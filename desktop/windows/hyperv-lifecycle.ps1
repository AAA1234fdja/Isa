#Requires -Version 5.1
<#
Experimental manual Hyper-V lifecycle prototype. This is NOT connected to ISA
execution, a broker, workspace copy/import, approvals or sandbox guarantees.
Run only on a disposable, trusted test host after reviewing the accompanying
sandbox documentation. A trusted local administrator must choose the image,
output root and existing switch. No disk or host workspace is deleted here.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Create', 'Destroy')]
    [string]$Action,
    [string]$BaseVhdx,
    [string]$BaseSha256,
    [string]$InstanceRoot,
    [Nullable[Guid]]$SwitchId,
    [Nullable[Guid]]$VmId,
    [Nullable[Guid]]$InstanceId
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'hyperv-policy.ps1')

function Get-TrustedLocalPath([string] $path, [bool] $directory) {
    if ([string]::IsNullOrWhiteSpace($path) -or -not [IO.Path]::IsPathRooted($path) -or $path -notmatch '^[a-zA-Z]:\\') {
        throw 'Use an absolute local drive path; UNC and device paths are not supported.'
    }
    $full = [IO.Path]::GetFullPath($path)
    $item = Get-Item -LiteralPath $full -Force -ErrorAction Stop
    if ([bool]$item.PSIsContainer -ne $directory) { throw 'Unexpected path type.' }
    # Reject visible reparse points in the existing ancestry. This is only a
    # best-effort check, not protection against same-user races or aliases.
    $current = $item
    while ($null -ne $current) {
        if (($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Reparse points are not supported.' }
        $parentPath = Split-Path -LiteralPath $current.FullName -Parent
        if ([string]::IsNullOrEmpty($parentPath) -or $parentPath -eq $current.FullName) { break }
        $current = Get-Item -LiteralPath $parentPath -Force -ErrorAction Stop
    }
    return $full
}

if ($Action -eq 'Create') {
    if ([string]::IsNullOrWhiteSpace($BaseVhdx) -or [string]::IsNullOrWhiteSpace($InstanceRoot) -or
        $BaseSha256 -notmatch '^[0-9a-fA-F]{64}$') {
        throw 'Create requires BaseVhdx, its expected BaseSha256, and an existing InstanceRoot.'
    }
    if ($null -ne $VmId -or $null -ne $InstanceId) { throw 'Do not supply VM or instance IDs when creating.' }
    $base = Get-TrustedLocalPath $BaseVhdx $false
    $root = Get-TrustedLocalPath $InstanceRoot $true
    if ([IO.Path]::GetExtension($base) -ine '.vhdx') { throw 'The base image must be a VHDX file.' }
    $actualHash = (Get-FileHash -LiteralPath $base -Algorithm SHA256).Hash
    if ($actualHash -ine $BaseSha256) { throw 'Base image hash does not match.' }
    $switch = $null
    if ($null -ne $SwitchId) {
        if ($SwitchId -eq [Guid]::Empty) { throw 'Switch ID must not be empty.' }
        $switch = Get-VMSwitch -Id $SwitchId -ErrorAction Stop
        if ($null -eq $switch -or [Guid]$switch.Id -ne $SwitchId) { throw 'Switch ID not found.' }
    }
    $id = [Guid]::NewGuid()
    $name = Get-IsaHyperVName $id
    $folder = Join-Path $root ($id.ToString('N'))
    $disk = Join-Path $folder 'guest-diff.vhdx'
    $network = if ($null -eq $switch) { 'no virtual switch' } else { 'existing switch ' + $switch.Name + ' (' + $switch.Id + ')' }
    $description = "Create and start experimental VM $name with base $base, differencing disk $disk and $network. Network may reach host, LAN and internet; no ISA workspace or agent is configured."
    if (-not $PSCmdlet.ShouldProcess($description, 'Create and start VM')) { return }
    if (Test-Path -LiteralPath $folder) { throw 'Instance folder already exists; refusing to reuse it.' }
    # The root must be access-controlled by the user. No attempt is made to
    # delete a partially created folder/disk if a later step fails.
    $null = New-Item -ItemType Directory -Path $folder -ErrorAction Stop
    $vm = $null
    try {
        $null = New-VHD -Path $disk -ParentPath $base -Differencing -ErrorAction Stop
        $arguments = @{ Name = $name; Generation = 2; MemoryStartupBytes = 2GB; VHDPath = $disk; Path = $folder; ErrorAction = 'Stop' }
        if ($null -ne $switch) { $arguments.SwitchName = [string]$switch.Name }
        $vm = New-VM @arguments
        Set-VM -VM $vm -Notes (Get-IsaHyperVNotes $id) -AutomaticStartAction Nothing -AutomaticCheckpointsEnabled $false -ErrorAction Stop
        $vm = Get-VM -Id $vm.Id -ErrorAction Stop
        if (-not (Test-IsaHyperVIdentity $vm ([Guid]$vm.Id) $id)) { throw 'Created VM identity could not be verified.' }
        # Emit ownership info before starting so it is available if startup fails.
        [pscustomobject]@{ InstanceId = $id; VmId = [Guid]$vm.Id; Name = $name; Disk = $disk; BaseSha256 = $actualHash; Network = $network; Status = 'Created; start requested'; SandboxReady = $false }
        Start-VM -VM $vm -ErrorAction Stop
    } catch {
        $knownVm = if ($null -eq $vm) { 'unknown' } else { [string]$vm.Id }
        Write-Warning "Creation/start failed. Instance: $id; VM ID: $knownVm; folder: $folder. Resources may remain; inspect Hyper-V Manager. No automatic cleanup was attempted."
        throw
    }
} else {
    if ($null -eq $VmId -or $null -eq $InstanceId -or $VmId -eq [Guid]::Empty -or $InstanceId -eq [Guid]::Empty) {
        throw 'Destroy requires the exact nonempty VmId and InstanceId from creation.'
    }
    if ($BaseVhdx -or $BaseSha256 -or $InstanceRoot -or $null -ne $SwitchId) { throw 'Create-only arguments are not accepted for destroy.' }
    $vm = Get-VM -Id $VmId -ErrorAction Stop
    if (-not (Test-IsaHyperVIdentity $vm $VmId $InstanceId)) { throw 'VM ID, instance name or ownership marker does not match; refusing cleanup.' }
    $description = "Turn off and remove VM $($vm.Name) ($VmId). Guest state is lost. Disk files and folders are NOT removed."
    if (-not $PSCmdlet.ShouldProcess($description, 'Destroy VM')) { return }
    if ([string]$vm.State -ne 'Off') { Stop-VM -VM $vm -TurnOff -Force -ErrorAction Stop }
    $vm = Get-VM -Id $VmId -ErrorAction Stop
    if (-not (Test-IsaHyperVIdentity $vm $VmId $InstanceId) -or [string]$vm.State -ne 'Off') {
        throw 'VM identity or stopped state changed; refusing removal.'
    }
    Remove-VM -VM $vm -Force -ErrorAction Stop
    [pscustomobject]@{ InstanceId = $InstanceId; VmId = $VmId; Status = 'VM removal requested'; DiskRemoved = $false; CleanupVerified = $false }
}

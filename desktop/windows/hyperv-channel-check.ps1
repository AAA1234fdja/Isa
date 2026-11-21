#Requires -Version 5.1
<#
Manual PowerShell Direct connection diagnostic for an ISA-labeled Hyper-V VM.
This is not an ISA broker, does not transfer files or dispatch commands, and
must not be used to treat guest responses as trusted or as isolation evidence.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)][Guid]$VmId,
    [Parameter(Mandatory = $true)][Guid]$InstanceId
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'hyperv-policy.ps1')
if ($PSVersionTable.PSEdition -ne 'Desktop') {
    throw 'Use Windows PowerShell 5.1 on the Hyper-V host for this prototype.'
}
if ($VmId -eq [Guid]::Empty -or $InstanceId -eq [Guid]::Empty) { throw 'Nonempty VM and instance IDs are required.' }
$vm = Get-VM -Id $VmId -ErrorAction Stop
if (-not (Test-IsaHyperVIdentity $vm $VmId $InstanceId) -or [string]$vm.State -ne 'Running') {
    throw 'VM identity does not match or VM is not running.'
}
$description = "Authenticate to guest VM $($vm.Name) ($VmId) using PowerShell Direct. Guest credentials must be dedicated to this disposable guest, not host credentials. The guest response is untrusted."
if (-not $PSCmdlet.ShouldProcess($description, 'Open guest diagnostic session')) { return }
$credential = Get-Credential -Message 'Enter credentials for the disposable guest only; do not use host credentials.'
if ($null -eq $credential) { throw 'No guest credential supplied.' }
# Recheck identity after the interactive prompt. Markers guard against mistakes,
# not an attacker with host access. Credentials authenticate the guest account;
# they do not attest guest integrity or authorize access to host files.
$vm = Get-VM -Id $VmId -ErrorAction Stop
if (-not (Test-IsaHyperVIdentity $vm $VmId $InstanceId) -or [string]$vm.State -ne 'Running') {
    throw 'VM identity or state changed; refusing connection.'
}
$session = $null
try {
    $session = New-PSSession -VMId $VmId -Credential $credential -ErrorAction Stop
    $reply = Invoke-Command -Session $session -ScriptBlock { 'isa-guest-diagnostic-v1' } -ErrorAction Stop
    if (@($reply).Count -ne 1 -or [string]$reply -cne 'isa-guest-diagnostic-v1') { throw 'Unexpected guest response.' }
    [pscustomobject]@{ VmId = $VmId; InstanceId = $InstanceId; GuestSessionOpened = $true; GuestTrusted = $false; WorkspaceTransferred = $false; SandboxReady = $false }
} finally {
    if ($null -ne $session) { Remove-PSSession -Session $session -ErrorAction SilentlyContinue }
    $credential = $null
}

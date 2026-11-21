#Requires -Version 5.1
# Mock-only tests: no credentials, real guest sessions or VM changes.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'hyperv-policy.ps1')
function Assert([bool]$condition, [string]$message) { if (-not $condition) { throw $message } }
$instanceId = [Guid]'c61d7a40-1993-4d0b-9107-e1f5c4dd1cb3'
$vmId = [Guid]'b48e5c5c-80cf-455e-ae66-3b3f6f27045b'
$global:IsaChannelVm = [pscustomobject]@{ Id = $vmId; Name = (Get-IsaHyperVName $instanceId); Notes = (Get-IsaHyperVNotes $instanceId); State = 'Running' }
$global:IsaCredentialCalls = 0
$global:IsaSessionCalls = 0
$global:IsaCloseCalls = 0
$global:IsaBadReply = $false
function Get-VM {
    [CmdletBinding()]
    param([Guid]$Id)
    if ($Id -ne $global:IsaChannelVm.Id) { throw 'VM not found' }
    return $global:IsaChannelVm
}
function Get-Credential {
    [CmdletBinding()]
    param([string]$Message)
    $global:IsaCredentialCalls++
    return [PSCredential]::new('fake-guest', (ConvertTo-SecureString 'test-only' -AsPlainText -Force))
}
function New-PSSession {
    [CmdletBinding()]
    param([Guid]$VMId, [PSCredential]$Credential)
    $global:IsaSessionCalls++
    return [pscustomobject]@{ Mock = $true }
}
function Invoke-Command {
    [CmdletBinding()]
    param($Session, [scriptblock]$ScriptBlock)
    if ($global:IsaBadReply) { return 'bad response' }
    return 'isa-guest-diagnostic-v1'
}
function Remove-PSSession {
    [CmdletBinding()]
    param($Session)
    $global:IsaCloseCalls++
}
foreach ($name in @('Get-VM', 'Get-Credential', 'New-PSSession', 'Invoke-Command', 'Remove-PSSession')) {
    Assert ((Get-Command $name).CommandType -eq 'Function') "Mock missing: $name"
}
$script = Join-Path $PSScriptRoot 'hyperv-channel-check.ps1'
$global:IsaChannelVm.Notes = 'foreign'
$rejected = $false
try { & $script -VmId $vmId -InstanceId $instanceId -Confirm:$false } catch { $rejected = $true }
Assert $rejected 'Foreign VM must be rejected.'
Assert ($global:IsaCredentialCalls -eq 0 -and $global:IsaSessionCalls -eq 0) 'Rejected VM requested credentials or opened session.'
$global:IsaChannelVm.Notes = Get-IsaHyperVNotes $instanceId
& $script -VmId $vmId -InstanceId $instanceId -WhatIf
Assert ($global:IsaCredentialCalls -eq 0 -and $global:IsaSessionCalls -eq 0) 'WhatIf requested credentials or opened session.'
$result = & $script -VmId $vmId -InstanceId $instanceId -Confirm:$false
Assert ($global:IsaSessionCalls -eq 1 -and $global:IsaCloseCalls -eq 1) 'Session was not closed.'
Assert ($result.GuestSessionOpened -and -not $result.GuestTrusted -and -not $result.WorkspaceTransferred -and -not $result.SandboxReady) 'Diagnostic overclaims readiness.'
$global:IsaBadReply = $true
$rejected = $false
try { & $script -VmId $vmId -InstanceId $instanceId -Confirm:$false } catch { $rejected = $true }
Assert ($rejected -and $global:IsaCloseCalls -eq 2) 'Unexpected guest response must fail and close the session.'
Write-Host 'PowerShell Direct diagnostic mock checks passed.'

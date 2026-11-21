#Requires -Version 5.1
# Mock-only tests. No Hyper-V calls or VM changes are made.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'hyperv-policy.ps1')
function Assert([bool]$condition, [string]$message) { if (-not $condition) { throw $message } }
$id = [Guid]'c61d7a40-1993-4d0b-9107-e1f5c4dd1cb3'
$vmId = [Guid]'b48e5c5c-80cf-455e-ae66-3b3f6f27045b'
$vm = [pscustomobject]@{ Id = $vmId; Name = (Get-IsaHyperVName $id); Notes = (Get-IsaHyperVNotes $id); State = 'Off' }
Assert (Test-IsaHyperVIdentity $vm $vmId $id) 'Expected exact identity.'
Assert (-not (Test-IsaHyperVIdentity $vm ([Guid]::NewGuid()) $id)) 'Wrong VM ID accepted.'
Assert (-not (Test-IsaHyperVIdentity $vm $vmId ([Guid]::NewGuid()))) 'Wrong instance ID accepted.'
$vm.Notes = 'foreign'; Assert (-not (Test-IsaHyperVIdentity $vm $vmId $id)) 'Wrong marker accepted.'
$vm.Notes = Get-IsaHyperVNotes $id
$vm.Name = 'other-vm'; Assert (-not (Test-IsaHyperVIdentity $vm $vmId $id)) 'Wrong name accepted.'
$vm.Name = Get-IsaHyperVName $id
Assert (-not (Test-IsaHyperVIdentity $vm ([Guid]::Empty) $id)) 'Empty VM ID accepted.'

$global:IsaMockVm = $vm
$global:IsaMockStopCalls = 0
$global:IsaMockRemoveCalls = 0
function Get-VM {
    [CmdletBinding()]
    param([Guid]$Id)
    if ($Id -ne $global:IsaMockVm.Id) { throw 'not found' }
    return $global:IsaMockVm
}
function Stop-VM {
    [CmdletBinding()]
    param($VM, [switch]$TurnOff, [switch]$Force)
    $global:IsaMockStopCalls++
    $global:IsaMockVm.State = 'Off'
}
function Remove-VM {
    [CmdletBinding()]
    param($VM, [switch]$Force)
    $global:IsaMockRemoveCalls++
}
foreach ($name in @('Get-VM', 'Stop-VM', 'Remove-VM')) {
    Assert ((Get-Command $name).CommandType -eq 'Function') "Mock missing: $name"
}
$helper = Join-Path $PSScriptRoot 'hyperv-lifecycle.ps1'
$global:IsaMockVm.Notes = 'foreign'
$rejected = $false
try { & $helper -Action Destroy -VmId $vmId -InstanceId $id -Confirm:$false } catch { $rejected = $true }
Assert $rejected 'Foreign VM was not rejected.'
Assert ($global:IsaMockStopCalls -eq 0 -and $global:IsaMockRemoveCalls -eq 0) 'Invalid identity performed an operation.'
$global:IsaMockVm.Notes = Get-IsaHyperVNotes $id
$global:IsaMockVm.State = 'Running'
& $helper -Action Destroy -VmId $vmId -InstanceId $id -WhatIf
Assert ($global:IsaMockStopCalls -eq 0 -and $global:IsaMockRemoveCalls -eq 0) 'WhatIf performed an operation.'
$result = & $helper -Action Destroy -VmId $vmId -InstanceId $id -Confirm:$false
Assert ($global:IsaMockStopCalls -eq 1 -and $global:IsaMockRemoveCalls -eq 1) 'Expected one stop and one remove.'
Assert ($result.DiskRemoved -eq $false -and $result.CleanupVerified -eq $false) 'Cleanup must not be overclaimed.'
Write-Host 'Hyper-V lifecycle mock checks passed.'

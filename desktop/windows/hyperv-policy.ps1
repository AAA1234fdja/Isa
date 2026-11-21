# Pure identification checks for the experimental Hyper-V lifecycle helper.
# These markers prevent accidental cross-instance operations; they are NOT an
# authentication mechanism against a malicious local administrator or process.
function Get-IsaHyperVName([Guid] $instanceId) {
    if ($instanceId -eq [Guid]::Empty) { throw 'Instance ID must not be empty.' }
    return 'ISA-Sandbox-' + $instanceId.ToString('N')
}
function Get-IsaHyperVNotes([Guid] $instanceId) {
    if ($instanceId -eq [Guid]::Empty) { throw 'Instance ID must not be empty.' }
    return 'ISA experimental instance v1: ' + $instanceId.ToString('D')
}
function Test-IsaHyperVIdentity($vm, [Guid] $vmId, [Guid] $instanceId) {
    if ($null -eq $vm -or $vmId -eq [Guid]::Empty -or $instanceId -eq [Guid]::Empty) { return $false }
    return ([Guid]$vm.Id -eq $vmId -and
        [string]$vm.Name -ceq (Get-IsaHyperVName $instanceId) -and
        [string]$vm.Notes -ceq (Get-IsaHyperVNotes $instanceId))
}

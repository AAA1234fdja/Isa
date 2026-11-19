//! Diagnostic Windows Sandbox configuration prototype; test-only and inert.
//!
//! No launch, mappings, logon command, workspace transfer or guest agent is
//! implemented. This requested configuration is not proof of effective policy.

/// Static diagnostic template: no user-controlled XML, host paths or commands.
/// Networking is intentionally enabled per the requested scope, exposing the
/// guest to potentially sensitive host/LAN services and enabling exfiltration.
/// MemoryInMB is a request, not an independently verified hard resource limit.
pub(crate) fn diagnostic_configuration() -> &'static str {
    "<Configuration>\n\
     <vGPU>Disable</vGPU>\n\
     <Networking>Enable</Networking>\n\
     <AudioInput>Disable</AudioInput>\n\
     <VideoInput>Disable</VideoInput>\n\
     <ProtectedClient>Enable</ProtectedClient>\n\
     <PrinterRedirection>Disable</PrinterRedirection>\n\
     <ClipboardRedirection>Disable</ClipboardRedirection>\n\
     <MemoryInMB>4096</MemoryInMB>\n\
     </Configuration>\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_wsb_is_fixed_and_explicit_without_shares_or_commands() {
        let config = diagnostic_configuration();
        assert_eq!(config, concat!(
            "<Configuration>\n",
            "<vGPU>Disable</vGPU>\n",
            "<Networking>Enable</Networking>\n",
            "<AudioInput>Disable</AudioInput>\n",
            "<VideoInput>Disable</VideoInput>\n",
            "<ProtectedClient>Enable</ProtectedClient>\n",
            "<PrinterRedirection>Disable</PrinterRedirection>\n",
            "<ClipboardRedirection>Disable</ClipboardRedirection>\n",
            "<MemoryInMB>4096</MemoryInMB>\n",
            "</Configuration>\n",
        ));
        assert!(!config.contains("MappedFolders"));
        assert!(!config.contains("LogonCommand"));
    }
}

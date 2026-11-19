//! Experimental opt-in Windows supervision, restricted to explicit endpoints.
//! Edits, undo, MCP and work tracking remain unavailable. Encrypted details
//! have a separate experimental opt-in; metadata events remain volatile.

pub(crate) fn audit_enabled() -> bool {
    cfg!(windows) && std::env::var("ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL").as_deref()==Ok("1") && std::env::var("ISA_WEB_AUDIT_ENCRYPTED").as_deref()==Ok("1")
}
pub(crate) fn audit_endpoint_allowed(path:&str,audit:bool)->bool {audit && path=="/api/actions/usage"}

pub(crate) fn shell_enabled() -> bool {
    cfg!(windows) && std::env::var("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL").as_deref() == Ok("1")
}
pub(crate) fn enabled() -> bool {
    cfg!(windows) && (shell_enabled() || std::env::var("ISA_WEB_WINDOWS_READ_ONLY").as_deref() == Ok("1"))
}
pub(crate) fn endpoint_allowed(path: &str, shell: bool) -> bool {
    matches!(path, "/api/exec/policy" | "/api/actions/prepare" | "/api/actions/run" | "/api/actions/cancel" | "/api/actions/events" | "/api/actions/workspaces")
    || (shell && matches!(path,"/api/exec/prepare" | "/api/exec/run" | "/api/exec/cancel" | "/api/actions/stop"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_read_only_gate_allows_only_explicit_endpoints() {
        for path in ["/api/exec/policy", "/api/actions/prepare", "/api/actions/run", "/api/actions/cancel", "/api/actions/events", "/api/actions/workspaces"] { assert!(endpoint_allowed(path,false)); }
        for path in ["/api/exec/prepare", "/api/exec/run", "/api/actions/stop", "/api/actions/usage", "/api/actions/sessions/read", "/api/actions/pool/start", "/api/actions/mcp/close", "/api/actions/unknown"] { assert!(!endpoint_allowed(path,false)); }
    }
    #[test]
    fn windows_audit_gate_exposes_only_usage_with_explicit_opt_in() {
        assert!(!audit_endpoint_allowed("/api/actions/usage",false));
        assert!(audit_endpoint_allowed("/api/actions/usage",true));
        for path in ["/api/actions/pool/start","/api/actions/mcp/close","/api/actions/unknown"] {assert!(!audit_endpoint_allowed(path,true));}
    }
    #[test]
    fn windows_shell_gate_does_not_allow_other_services() {
        for path in ["/api/exec/prepare","/api/exec/run","/api/exec/cancel","/api/actions/stop"] {assert!(endpoint_allowed(path,true));}
        for path in ["/api/actions/usage","/api/actions/sessions/read","/api/actions/pool/start","/api/actions/mcp/close","/api/actions/unknown"] {assert!(!endpoint_allowed(path,true));}
    }
}

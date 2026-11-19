//! Conservative SDDL construction only; not a complete ACL verification policy.
use std::io;
pub(super) fn private_sddl(sid:&str, directory:bool)->io::Result<String> {
    if !sid.starts_with("S-1-") || sid.len()>184 || sid[2..].split('-').any(|n|n.is_empty()||!n.bytes().all(|b|b.is_ascii_digit())) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid user SID"));
    }
    Ok(format!("O:{sid}D:P(A;{};FA;;;{sid})",if directory {"OICI"} else {""}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_acl_policy_constrains_sid_and_sets_private_dacl() {
        assert_eq!(private_sddl("S-1-5-21-123",true).unwrap(),"O:S-1-5-21-123D:P(A;OICI;FA;;;S-1-5-21-123)");
        assert_eq!(private_sddl("S-1-5-21-123",false).unwrap(),"O:S-1-5-21-123D:P(A;;FA;;;S-1-5-21-123)");
        for invalid in ["", "S-1-5--21", "S-1-5-21)D:(A;;FA;;;WD", "S-2-1", "S-1-5-", "S-1-💡"] {assert!(private_sddl(invalid,true).is_err());}
    }
}

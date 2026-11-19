//! Build arguments for Windows PowerShell's UTF-16LE EncodedCommand mode.
//! This does not approve, sandbox, or make the supplied script safe to execute.
//! Base64 is not encryption: the script can be visible in the process command line.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::io;

pub(super) fn arguments(script: &str) -> io::Result<Vec<String>> {
    if script.trim().is_empty() || script.len()>4096 || script.chars().any(|c| (c.is_control() && c!='\n' && c!='\t') || matches!(c,'\u{061c}'|'\u{200e}'|'\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid PowerShell script"));
    }
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    Ok(vec![
        "-NoLogo".into(),
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-EncodedCommand".into(),
        STANDARD.encode(bytes),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_command_encodes_exact_utf16le_without_interpretation() {
        for script in [
            "Write-Output 'hello'",
            "a\"; $(Write-Output 'é🙂')\n#test",
            "C:\\path with spaces\\",
        ] {
            let args = arguments(script).unwrap();
            assert_eq!(args.len(), 5);
            assert_eq!(args[3], "-EncodedCommand");
            assert!(!args[..4].iter().any(|a| a.contains(script)));
            let decoded = STANDARD.decode(&args[4]).unwrap();
            let expected: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert_eq!(decoded, expected);
        }
    }
    #[test]
    fn powershell_command_rejects_unreviewable_or_oversized_scripts() {
        for script in ["", " \n", "hello\0bad", "x\u{202e}y", "x\u{1b}y"] {
            assert!(arguments(script).is_err());
        }
        assert!(arguments(&"x".repeat(4097)).is_err());
        assert!(arguments("Write-Output 'ok'\n").is_ok());
    }
}

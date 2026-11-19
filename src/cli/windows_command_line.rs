//! Windows CRT-style argument quoting; does not interpret shell syntax.
//! Some Windows programs use a different parser. Do not use this to escape a
//! script into cmd.exe or PowerShell source; pass scripts via a dedicated API.
use std::io;
const MAX_COMMAND_LINE:usize=32767; // includes the terminating NUL

fn append_arg(out:&mut Vec<u16>, arg:&[u16]) -> io::Result<()> {
    if arg.contains(&0) { return Err(io::Error::new(io::ErrorKind::InvalidInput,"argument contains NUL")); }
    if arg.len() >= MAX_COMMAND_LINE { return Err(io::Error::new(io::ErrorKind::InvalidInput,"Windows argument is too long")); }
    out.push(b'"' as u16);
    let mut slashes=0usize;
    for &ch in arg {
        if ch==b'\\' as u16 { slashes+=1; continue; }
        if ch==b'"' as u16 {
            out.extend(std::iter::repeat_n(b'\\' as u16,slashes*2+1));
        } else {
            out.extend(std::iter::repeat_n(b'\\' as u16,slashes));
        }
        slashes=0;out.push(ch);
        if out.len()>MAX_COMMAND_LINE { return Err(io::Error::new(io::ErrorKind::InvalidInput,"Windows command line is too long")); }
    }
    out.extend(std::iter::repeat_n(b'\\' as u16,slashes*2));
    out.push(b'"' as u16);
    if out.len()>=MAX_COMMAND_LINE { return Err(io::Error::new(io::ErrorKind::InvalidInput,"Windows command line is too long")); }
    Ok(())
}

/// Build a NUL-terminated command line from an executable and independent args.
/// Quoting follows common Windows CRT parsing, not shell metacharacter escaping.
pub(super) fn build(program:&[u16], args:&[Vec<u16>]) -> io::Result<Vec<u16>> {
    if program.is_empty() || program.contains(&(b'"' as u16)) { return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid executable path")); }
    let mut result=Vec::new();append_arg(&mut result,program)?;
    for arg in args { result.push(b' ' as u16);append_arg(&mut result,arg)?; }
    result.push(0);Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arg(s:&str)->Vec<u16> {s.encode_utf16().collect()}
    fn quoted(s:&str)->String {let mut v=Vec::new();append_arg(&mut v,&arg(s)).unwrap();String::from_utf16(&v).unwrap()}
    #[test] fn windows_command_line_known_quoting_cases() {
        assert_eq!(quoted(""),"\"\"");
        assert_eq!(quoted("hello world"),"\"hello world\"");
        assert_eq!(quoted("a\"b"),"\"a\\\"b\"");
        assert_eq!(quoted("a\\\"b"),"\"a\\\\\\\"b\"");
        assert_eq!(quoted("C:\\folder\\"),"\"C:\\folder\\\\\"");
        let line=build(&arg("C:\\Program Files\\test.exe"),&[arg("a b"),arg("é🙂")]).unwrap();
        assert_eq!(String::from_utf16(&line[..line.len()-1]).unwrap(),"\"C:\\Program Files\\test.exe\" \"a b\" \"é🙂\"");
    }
    // Reference decoder for one always-quoted argument, following the CRT rule
    // for runs of backslashes immediately before a quote.
    fn decode_one(encoded:&[u16])->Vec<u16> {
        assert_eq!(encoded[0],b'"' as u16);
        let mut out=Vec::new();let mut i=1;
        while i<encoded.len() {
            let mut slashes=0;while i<encoded.len() && encoded[i]==b'\\' as u16 {slashes+=1;i+=1;}
            if i<encoded.len() && encoded[i]==b'"' as u16 {
                out.extend(std::iter::repeat_n(b'\\' as u16,slashes/2));
                i+=1;if slashes%2==0 {assert_eq!(i,encoded.len());return out;}out.push(b'"' as u16);
            } else {out.extend(std::iter::repeat_n(b'\\' as u16,slashes));if i<encoded.len(){out.push(encoded[i]);i+=1;}}
        }
        panic!("missing closing quote")
    }
    #[test] fn windows_command_line_roundtrips_mixed_backslashes_and_quotes() {
        let alphabet=[b'a' as u16,b' ' as u16,b'\\' as u16,b'"' as u16,0x00e9];
        for size in 0..=5 { for mut n in 0..alphabet.len().pow(size) {
            let mut original=Vec::new();for _ in 0..size {original.push(alphabet[n%alphabet.len()]);n/=alphabet.len();}
            let mut encoded=Vec::new();append_arg(&mut encoded,&original).unwrap();assert_eq!(decode_one(&encoded),original);
        }}
    }
    #[test] fn windows_command_line_rejects_nul_and_oversize() {
        assert!(build(&[],&[]).is_err());assert!(build(&arg("x\".exe"),&[]).is_err());
        assert!(build(&[b'x' as u16,0],&[]).is_err());assert!(build(&arg("x"),&[vec![0]]).is_err());
        assert!(build(&arg("x"),&[vec![b'a' as u16;32767]]).is_err());
        assert!(build(&arg("x"),&[vec![b'\\' as u16;32767]]).is_err());
    }
}

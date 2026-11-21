//! Staged, bounded parsing of untrusted guest messages. No peer authentication,
//! transport, guest implementation, execution or import is provided here.
use serde::Deserialize;

const MAX_MESSAGE: usize = 64 * 1024;
const MAX_OUTPUT: usize = 16 * 1024;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    instance: String,
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    code: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Message {
    Ready,
    Stdout { text: String },
    Stderr { text: String },
    Exit { code: i32 },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    Invalid,
    Version,
    WrongInstance,
    TooLarge,
}

/// Parsing a matching instance ID is not authentication: an untrusted guest
/// can repeat it. A future transport must independently authenticate the peer.
pub(crate) fn parse(bytes: &[u8], expected_instance: &str) -> Result<Message, ProtocolError> {
    if bytes.len() > MAX_MESSAGE {
        return Err(ProtocolError::TooLarge);
    }
    if expected_instance.len() != 32
        || !expected_instance
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(ProtocolError::Invalid);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| ProtocolError::Invalid)?;
    if envelope.version != 1 {
        return Err(ProtocolError::Version);
    }
    if envelope.instance != expected_instance {
        return Err(ProtocolError::WrongInstance);
    }
    match (envelope.kind.as_str(), envelope.text, envelope.code) {
        ("ready", None, None) => Ok(Message::Ready),
        ("exit", None, Some(code)) => Ok(Message::Exit { code }),
        ("stdout" | "stderr", Some(text), None) if text.len() > MAX_OUTPUT => {
            Err(ProtocolError::TooLarge)
        }
        ("stdout", Some(text), None) => Ok(Message::Stdout { text }),
        ("stderr", Some(text), None) => Ok(Message::Stderr { text }),
        _ => Err(ProtocolError::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn accepts_only_known_bounded_messages_for_the_expected_instance() {
        let ready = format!(r#"{{"version":1,"instance":"{ID}","type":"ready"}}"#);
        assert_eq!(parse(ready.as_bytes(), ID), Ok(Message::Ready));
        let exit = format!(r#"{{"version":1,"instance":"{ID}","type":"exit","code":1}}"#);
        assert_eq!(parse(exit.as_bytes(), ID), Ok(Message::Exit { code: 1 }));
        assert_eq!(
            parse(ready.as_bytes(), "ffffffffffffffffffffffffffffffff"),
            Err(ProtocolError::WrongInstance)
        );
        assert_eq!(parse(ready.as_bytes(), "bad"), Err(ProtocolError::Invalid));
        let version = ready.replace("\"version\":1", "\"version\":2");
        assert_eq!(parse(version.as_bytes(), ID), Err(ProtocolError::Version));
        let extra =
            format!(r#"{{"version":1,"instance":"{ID}","type":"ready","unexpected":true}}"#);
        assert_eq!(parse(extra.as_bytes(), ID), Err(ProtocolError::Invalid));
        let duplicate = format!(r#"{{"version":1,"version":1,"instance":"{ID}","type":"ready"}}"#);
        assert_eq!(parse(duplicate.as_bytes(), ID), Err(ProtocolError::Invalid));
        let unknown = format!(r#"{{"version":1,"instance":"{ID}","type":"approve"}}"#);
        assert_eq!(parse(unknown.as_bytes(), ID), Err(ProtocolError::Invalid));
    }

    #[test]
    fn fragmented_frames_are_parsed_only_after_complete_decoding() {
        use super::super::windows_sandbox_frame::{Decoder, Encoder};
        let payload =
            format!(r#"{{"version":1,"instance":"{ID}","type":"stdout","text":"hello"}}"#);
        let frame = Encoder::new().encode(payload.as_bytes()).unwrap();
        let mut decoder = Decoder::new();
        assert!(decoder.push(&frame[..3]).unwrap().is_empty());
        let decoded = decoder.push(&frame[3..]).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(
            parse(&decoded[0], ID),
            Ok(Message::Stdout {
                text: "hello".into()
            })
        );
    }

    #[test]
    fn caps_encoded_message_and_output_bytes() {
        assert_eq!(
            parse(&vec![b'x'; MAX_MESSAGE + 1], ID),
            Err(ProtocolError::TooLarge)
        );
        let output = format!(
            r#"{{"version":1,"instance":"{ID}","type":"stdout","text":"{}"}}"#,
            "x".repeat(MAX_OUTPUT + 1)
        );
        assert_eq!(parse(output.as_bytes(), ID), Err(ProtocolError::TooLarge));
    }
}

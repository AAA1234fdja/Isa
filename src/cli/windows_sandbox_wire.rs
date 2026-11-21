//! Staged transfer message adapter for a future authenticated guest transport.
//! This is NOT the transport, authentication, guest agent, or an import path.
//! The caller must bound reads before passing them to this adapter and must
//! authenticate the peer separately. Guest-provided data remains untrusted.
use super::windows_sandbox_frame::Decoder;
use super::windows_sandbox_transfer::{Item, ReceivedWorkspace, Receiver};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    instance: String,
    kind: String,
    entries: Option<Vec<WireItem>>,
    data: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireItem {
    path: String,
    kind: String,
    size: Option<u64>,
    sha256: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WireError {
    Invalid,
    Framing,
    Transfer,
    Incomplete,
    Failed,
}

pub(crate) struct TransferWire {
    instance: String,
    decoder: Decoder,
    receiver: Option<Receiver>,
    result: Option<ReceivedWorkspace>,
    ended: bool,
    failed: bool,
}

fn digest(value: &str) -> Result<[u8; 32], WireError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(WireError::Invalid);
    }
    let mut output = [0; 32];
    for (i, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| WireError::Invalid)?;
    }
    Ok(output)
}

impl TransferWire {
    pub(crate) fn new(instance: &str) -> Result<Self, WireError> {
        if instance.len() != 32
            || !instance
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(WireError::Invalid);
        }
        Ok(Self {
            instance: instance.into(),
            decoder: Decoder::new(),
            receiver: None,
            result: None,
            ended: false,
            failed: false,
        })
    }

    /// Accept a bounded transport read. Any malformed or unexpected frame
    /// irreversibly fails the session; previously completed data is discarded.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        if self.failed {
            return Err(WireError::Failed);
        }
        let frames = match self.decoder.push(bytes) {
            Ok(frames) => frames,
            Err(_) => {
                self.failed = true;
                self.result = None;
                self.receiver = None;
                return Err(WireError::Framing);
            }
        };
        for frame in frames {
            if let Err(err) = self.accept(&frame) {
                self.failed = true;
                self.result = None;
                self.receiver = None;
                return Err(err);
            }
        }
        Ok(())
    }

    fn accept(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        if self.ended {
            return Err(WireError::Invalid);
        }
        let msg: Envelope = serde_json::from_slice(bytes).map_err(|_| WireError::Invalid)?;
        if msg.version != 1 || msg.instance != self.instance {
            return Err(WireError::Invalid);
        }
        match (msg.kind.as_str(), msg.entries, msg.data) {
            ("inventory", Some(entries), None) if self.receiver.is_none() => {
                let mut items = Vec::with_capacity(entries.len());
                for entry in entries {
                    match (entry.kind.as_str(), entry.size, entry.sha256) {
                        ("directory", None, None) => {
                            items.push(Item::Directory { path: entry.path })
                        }
                        ("file", Some(size), Some(hash)) => items.push(Item::File {
                            path: entry.path,
                            size,
                            digest: digest(&hash)?,
                        }),
                        _ => return Err(WireError::Invalid),
                    }
                }
                self.receiver = Some(Receiver::new(&items).map_err(|_| WireError::Transfer)?);
            }
            ("chunk", None, Some(data)) => {
                // 32 KiB is the receiver limit; reject oversized text before decoding.
                if data.len() > 43_692 {
                    return Err(WireError::Invalid);
                }
                let bytes = STANDARD
                    .decode(data.as_bytes())
                    .map_err(|_| WireError::Invalid)?;
                self.receiver
                    .as_mut()
                    .ok_or(WireError::Invalid)?
                    .push(&bytes)
                    .map_err(|_| WireError::Transfer)?;
            }
            ("end_file", None, None) => {
                self.receiver
                    .as_mut()
                    .ok_or(WireError::Invalid)?
                    .end_file()
                    .map_err(|_| WireError::Transfer)?;
            }
            ("end_transfer", None, None) => {
                let receiver = self.receiver.take().ok_or(WireError::Invalid)?;
                self.result = Some(receiver.finish().map_err(|_| WireError::Transfer)?);
                self.ended = true;
            }
            _ => return Err(WireError::Invalid),
        }
        Ok(())
    }

    /// Require both an explicit completed transfer and an untruncated stream.
    pub(crate) fn finish(mut self) -> Result<ReceivedWorkspace, WireError> {
        if self.failed {
            return Err(WireError::Failed);
        }
        self.decoder.finish().map_err(|_| WireError::Framing)?;
        if !self.ended {
            return Err(WireError::Incomplete);
        }
        self.result.ok_or(WireError::Incomplete)
    }
}

#[cfg(test)]
mod tests {
    use super::super::windows_sandbox_frame::Encoder;
    use super::*;
    use sha2::{Digest, Sha256};
    const ID: &str = "0123456789abcdef0123456789abcdef";
    fn encoded(messages: &[serde_json::Value]) -> Vec<u8> {
        let mut encoder = Encoder::new();
        let mut out = Vec::new();
        for message in messages {
            out.extend(encoder.encode(message.to_string().as_bytes()).unwrap());
        }
        out
    }
    fn feed(wire: &mut TransferWire, data: &[u8]) -> Result<(), WireError> {
        for part in data.chunks(7) {
            wire.push(part)?;
        }
        Ok(())
    }
    #[test]
    fn connects_framed_chunks_to_receiver_and_rejects_trailing_messages() {
        let hash = format!("{:x}", Sha256::digest(b"hello"));
        let msgs = [
            serde_json::json!({"version":1,"instance":ID,"kind":"inventory","entries":[{"path":"file.txt","kind":"file","size":5,"sha256":hash}]}),
            serde_json::json!({"version":1,"instance":ID,"kind":"chunk","data":"aGVsbG8="}),
            serde_json::json!({"version":1,"instance":ID,"kind":"end_file"}),
            serde_json::json!({"version":1,"instance":ID,"kind":"end_transfer"}),
        ];
        let data = encoded(&msgs);
        let mut wire = TransferWire::new(ID).unwrap();
        feed(&mut wire, &data).unwrap();
        let files = wire.finish().unwrap().into_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "file.txt");
        assert_eq!(files[0].contents, b"hello");
        let mut wire = TransferWire::new(ID).unwrap();
        let mut duplicate = data;
        duplicate.extend(encoded(&[
            serde_json::json!({"version":1,"instance":ID,"kind":"end_transfer"}),
        ]));
        assert_eq!(feed(&mut wire, &duplicate), Err(WireError::Invalid));
        assert_eq!(wire.finish(), Err(WireError::Failed));
    }
    #[test]
    fn rejects_wrong_instance_early_chunks_and_incomplete_or_truncated_input() {
        let mut wire = TransferWire::new(ID).unwrap();
        let bytes = encoded(&[
            serde_json::json!({"version":1,"instance":"ffffffffffffffffffffffffffffffff","kind":"inventory","entries":[]}),
        ]);
        assert_eq!(feed(&mut wire, &bytes), Err(WireError::Invalid));
        let mut wire = TransferWire::new(ID).unwrap();
        let bytes =
            encoded(&[serde_json::json!({"version":1,"instance":ID,"kind":"chunk","data":"YQ=="})]);
        assert_eq!(feed(&mut wire, &bytes), Err(WireError::Invalid));
        let mut wire = TransferWire::new(ID).unwrap();
        wire.push(&[0, 0]).unwrap();
        assert_eq!(wire.finish(), Err(WireError::Framing));
        let wire = TransferWire::new(ID).unwrap();
        assert_eq!(wire.finish(), Err(WireError::Incomplete));
    }
}

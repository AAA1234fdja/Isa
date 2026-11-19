//! Select whether the server serves live development files or embedded assets.
//! This does not authenticate requests or make the server a desktop application.
use std::{ffi::OsStr, io, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssetMode {
    Live,
    Embedded,
}

impl AssetMode {
    pub(crate) fn parse(value: Option<&OsStr>) -> io::Result<Self> {
        match value {
            None => Ok(Self::Live),
            Some(value) if value == OsStr::new("live") => Ok(Self::Live),
            Some(value) if value == OsStr::new("embedded") => Ok(Self::Embedded),
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ISA_ASSET_MODE must be 'live' or 'embedded'",
            )),
        }
    }

    pub(crate) fn read(self, path: &Path, embedded: &[u8]) -> Vec<u8> {
        match self {
            Self::Live => std::fs::read(path).unwrap_or_else(|_| embedded.to_vec()),
            Self::Embedded => embedded.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_defaults_to_live_and_rejects_unknown_values() {
        assert_eq!(AssetMode::parse(None).unwrap(), AssetMode::Live);
        assert_eq!(
            AssetMode::parse(Some(OsStr::new("live"))).unwrap(),
            AssetMode::Live
        );
        assert_eq!(
            AssetMode::parse(Some(OsStr::new("embedded"))).unwrap(),
            AssetMode::Embedded
        );
        for value in ["", "EMBEDDED", "embedded ", "typo"] {
            assert!(AssetMode::parse(Some(OsStr::new(value))).is_err());
        }
    }

    #[test]
    fn embedded_mode_ignores_files_and_live_mode_uses_them() {
        let root = std::env::temp_dir().join(format!(
            "isa-ui-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("index.html");
        std::fs::write(&file, b"live").unwrap();
        assert_eq!(AssetMode::Live.read(&file, b"embedded"), b"live");
        assert_eq!(AssetMode::Embedded.read(&file, b"embedded"), b"embedded");
        std::fs::remove_file(&file).unwrap();
        assert_eq!(AssetMode::Live.read(&file, b"embedded"), b"embedded");
        assert_eq!(AssetMode::Embedded.read(&file, b"embedded"), b"embedded");
        std::fs::remove_dir(&root).unwrap();
    }
}

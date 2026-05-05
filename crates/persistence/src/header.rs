use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use thiserror::Error;

pub const SAVE_MAGIC: [u8; 4] = *b"TILE";
pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub game_version: String,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: u64,
    pub seed: u64,
    pub world_size: (i32, i32),
}

impl SaveHeader {
    pub fn new(seed: u64, world_size: (i32, i32)) -> Self {
        Self {
            magic: SAVE_MAGIC,
            version: SAVE_VERSION,
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: current_unix_seconds(),
            seed,
            world_size,
        }
    }
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("invalid magic bytes: expected {expected:?}, got {got:?}")]
    BadMagic { expected: [u8; 4], got: [u8; 4] },
    #[error("unsupported save version {0}; expected {SAVE_VERSION}")]
    UnsupportedVersion(u32),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("deserialization error: {0}")]
    Decode(#[from] bincode::Error),
}

pub fn write_header<W: Write>(writer: &mut W, header: &SaveHeader) -> Result<(), SaveError> {
    let bytes = bincode::serialize(header)?;
    writer.write_all(&bytes)?;
    Ok(())
}

pub fn read_header<R: Read>(reader: &mut R) -> Result<SaveHeader, SaveError> {
    let header: SaveHeader = bincode::deserialize_from(reader)?;
    validate_header(&header)?;
    Ok(header)
}

pub fn validate_header(header: &SaveHeader) -> Result<(), SaveError> {
    if header.magic != SAVE_MAGIC {
        return Err(SaveError::BadMagic {
            expected: SAVE_MAGIC,
            got: header.magic,
        });
    }
    if header.version != SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion(header.version));
    }
    Ok(())
}

fn current_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_header_roundtrip() {
        let original = SaveHeader::new(987654321, (128, 128));
        let mut buf = Vec::new();
        write_header(&mut buf, &original).expect("write failed");

        let mut cursor = Cursor::new(buf);
        let loaded = read_header(&mut cursor).expect("read failed");

        assert_eq!(loaded.magic, SAVE_MAGIC);
        assert_eq!(loaded.version, SAVE_VERSION);
        assert_eq!(loaded.seed, 987654321);
        assert_eq!(loaded.world_size, (128, 128));
    }

    #[test]
    fn test_bad_magic_rejected() {
        let mut bad = SaveHeader::new(1, (1, 1));
        bad.magic = *b"NOPE";
        let mut buf = Vec::new();
        // Serialize directly without write_header (skips validation)
        bincode::serialize_into(&mut buf, &bad).unwrap();

        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).expect_err("should fail");
        assert!(
            matches!(err, SaveError::BadMagic { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_wrong_version_rejected() {
        let mut wrong = SaveHeader::new(1, (1, 1));
        wrong.version = 99;
        let mut buf = Vec::new();
        bincode::serialize_into(&mut buf, &wrong).unwrap();

        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).expect_err("should fail");
        assert!(
            matches!(err, SaveError::UnsupportedVersion(99)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_validate_header_directly() {
        let h = SaveHeader::new(42, (64, 64));
        assert!(validate_header(&h).is_ok());
    }
}

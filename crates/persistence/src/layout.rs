use std::path::{Path, PathBuf};

/// Manages the directory layout of a save slot.
///
/// ```text
/// <root>/
/// ├── header.bin
/// ├── world.bin
/// ├── worldgen/
/// ├── chunks/
/// ├── entities.bin
/// └── reactions/
/// ```
pub struct SaveLayout {
    root: PathBuf,
}

impl SaveLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn header_path(&self) -> PathBuf {
        self.root.join("header.bin")
    }

    pub fn world_path(&self) -> PathBuf {
        self.root.join("world.bin")
    }

    pub fn entities_path(&self) -> PathBuf {
        self.root.join("entities.bin")
    }

    pub fn chunks_dir(&self) -> PathBuf {
        self.root.join("chunks")
    }

    pub fn chunk_path(&self, cx: i32, cy: i32, cz: i32) -> PathBuf {
        self.chunks_dir().join(format!("{cx}_{cy}_{cz}.bin"))
    }

    pub fn worldgen_dir(&self) -> PathBuf {
        self.root.join("worldgen")
    }

    pub fn reactions_dir(&self) -> PathBuf {
        self.root.join("reactions")
    }

    /// Create all required subdirectories.
    pub fn create_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.chunks_dir())?;
        std::fs::create_dir_all(self.worldgen_dir())?;
        std::fs::create_dir_all(self.reactions_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_dirs() {
        let tmp = std::env::temp_dir().join(format!(
            "tile_engine_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        let layout = SaveLayout::new(&tmp);
        layout.create_dirs().expect("create_dirs failed");

        assert!(layout.chunks_dir().is_dir());
        assert!(layout.worldgen_dir().is_dir());
        assert!(layout.reactions_dir().is_dir());

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_chunk_path_format() {
        let layout = SaveLayout::new("/save");
        assert_eq!(
            layout.chunk_path(3, -2, 1),
            PathBuf::from("/save/chunks/3_-2_1.bin")
        );
    }

    #[test]
    fn test_header_path() {
        let layout = SaveLayout::new("/save");
        assert_eq!(layout.header_path(), PathBuf::from("/save/header.bin"));
    }
}

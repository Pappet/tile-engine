use bevy_ecs::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter};

use crate::header::SaveError;
use crate::layout::SaveLayout;

/// Marks a Bevy Component as serializable for entity persistence (Bibel §11.8).
///
/// Each concrete type gets a stable numeric TYPE_ID. Must not change between
/// versions; add a migration if the schema changes.
pub trait Persistent: Component + Serialize + DeserializeOwned {
    const TYPE_ID: u32;

    fn serialize_component(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
}

/// Wire format for one entity: a list of (TYPE_ID, serialized bytes) pairs.
/// Renderer components (Sprite, Transform) are never included.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializedEntity {
    pub components: Vec<(u32, Vec<u8>)>,
}

impl SerializedEntity {
    pub fn new() -> Self {
        Self { components: vec![] }
    }

    pub fn push<P: Persistent>(&mut self, component: &P) -> Result<(), bincode::Error> {
        let bytes = component.serialize_component()?;
        self.components.push((P::TYPE_ID, bytes));
        Ok(())
    }

    /// Decode the component with the given TYPE_ID if present.
    pub fn get<P: Persistent>(&self) -> Option<P> {
        self.components
            .iter()
            .find(|(id, _)| *id == P::TYPE_ID)
            .and_then(|(_, bytes)| bincode::deserialize(bytes).ok())
    }
}

impl Default for SerializedEntity {
    fn default() -> Self {
        Self::new()
    }
}

/// Write entity list to `entities.bin`.
pub fn save_entities(layout: &SaveLayout, entities: &[SerializedEntity]) -> Result<(), SaveError> {
    let f = fs::File::create(layout.entities_path())?;
    bincode::serialize_into(BufWriter::new(f), entities)?;
    Ok(())
}

/// Read entity list from `entities.bin`. Returns empty vec if file absent.
pub fn load_entities(layout: &SaveLayout) -> Result<Vec<SerializedEntity>, SaveError> {
    let path = layout.entities_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let f = fs::File::open(path)?;
    let entities = bincode::deserialize_from(BufReader::new(f))?;
    Ok(entities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    // Stub component for tests — doesn't need bevy Component macro in unit tests.
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Component)]
    struct MockSource {
        rate: u8,
        kind: u32,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Component)]
    struct MockDrain {
        rate: u8,
    }

    impl Persistent for MockSource {
        const TYPE_ID: u32 = 101;
    }

    impl Persistent for MockDrain {
        const TYPE_ID: u32 = 102;
    }

    fn tmp_dir(id: &str) -> String {
        std::env::temp_dir()
            .join(format!("tile_p53_{}_{id}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn test_serialize_entity_roundtrip() {
        let src = MockSource { rate: 5, kind: 1 };
        let mut e = SerializedEntity::new();
        e.push(&src).unwrap();
        let recovered: MockSource = e.get().unwrap();
        assert_eq!(recovered, src);
    }

    #[test]
    fn test_wrong_type_id_returns_none() {
        let drain = MockDrain { rate: 3 };
        let mut e = SerializedEntity::new();
        e.push(&drain).unwrap();
        let recovered: Option<MockSource> = e.get();
        assert!(recovered.is_none());
    }

    #[test]
    fn test_save_load_entities() {
        let slot = tmp_dir("save_load");
        let layout = SaveLayout::new(&slot);
        layout.create_dirs().unwrap();

        let src = MockSource { rate: 7, kind: 2 };
        let drain = MockDrain { rate: 4 };

        let mut e1 = SerializedEntity::new();
        e1.push(&src).unwrap();

        let mut e2 = SerializedEntity::new();
        e2.push(&drain).unwrap();

        save_entities(&layout, &[e1, e2]).unwrap();
        let loaded = load_entities(&layout).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].get::<MockSource>().unwrap(), src);
        assert_eq!(loaded[1].get::<MockDrain>().unwrap(), drain);

        std::fs::remove_dir_all(&slot).ok();
    }

    #[test]
    fn test_load_entities_missing_file_returns_empty() {
        let slot = tmp_dir("missing");
        let layout = SaveLayout::new(&slot);
        layout.create_dirs().unwrap();

        let result = load_entities(&layout).unwrap();
        assert!(result.is_empty());

        std::fs::remove_dir_all(&slot).ok();
    }

    #[test]
    fn test_no_sprite_data_in_save() {
        // Verify that only registered Persistent components end up in the file.
        // Sprite/Transform are not Persistent → can't be added → file has no sprite data.
        let slot = tmp_dir("no_sprite");
        let layout = SaveLayout::new(&slot);
        layout.create_dirs().unwrap();

        let src = MockSource { rate: 1, kind: 0 };
        let mut e = SerializedEntity::new();
        e.push(&src).unwrap();
        save_entities(&layout, &[e]).unwrap();

        let bytes = std::fs::read(layout.entities_path()).unwrap();
        // "Sprite" as bytes must not appear in the file.
        let sprite_bytes = b"Sprite";
        assert!(
            !bytes.windows(sprite_bytes.len()).any(|w| w == sprite_bytes),
            "entities.bin must not contain Sprite data"
        );

        std::fs::remove_dir_all(&slot).ok();
    }
}

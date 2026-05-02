// core: data model, coordinates, World-API

pub mod activity;
pub mod chunk;
pub mod coords;
pub mod liquid;
pub mod material;
pub mod world;

pub use material::{
    ElementId, MAT_AIR, Material, MaterialFlags, MaterialId, MaterialRegistry, builtin_materials,
};

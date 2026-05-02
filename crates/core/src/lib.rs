// core: data model, coordinates, World-API

pub mod activity;
pub mod chunk;
pub mod coords;
pub mod material;
pub mod world;
pub mod liquid;

pub use material::{
    builtin_materials, ElementId, MAT_AIR, Material, MaterialFlags, MaterialId, MaterialRegistry,
};

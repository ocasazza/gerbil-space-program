pub mod model;
pub mod names;
pub mod parts;
pub mod plugin;
pub mod visuals;

pub use model::{
    AerodynamicLoads3d, AerodynamicProfile, MassProperties3d, RigidModule3d, ShipAssembly3d,
    StructuralJoint,
};
pub use parts::{
    generate_part, generate_ship, GeneratedShip, Manufacturer, PartSlot, Rarity, ShipPart,
    StatModifiers,
};
pub use plugin::{GeneratedShipComponent, ShipGenConfig, ShipGenPlugin};
pub use visuals::{draw_ship_visuals, spawn_ship_visual, ShipVisual};

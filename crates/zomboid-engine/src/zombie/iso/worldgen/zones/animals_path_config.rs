//! zombie.iso.worldgen.zones.AnimalsPathConfig record.

use crate::java;

/// zombie.iso.worldgen.zones.AnimalsPathConfig (animalType, count, chance, points, radius, extension,
/// extensionChance): one animal's migration paths - the animal type, how many paths, the chance one grows
/// at all, its waypoint coordinates and radii, and the chance each waypoint extends.
#[derive(Clone, Debug)]
pub struct AnimalsPathConfig {
    pub animal_type: String,
    pub count: i32,
    pub chance: f32,
    pub points: Vec<i32>,
    pub radius: Vec<i32>,
    pub extension: Vec<i32>,
    pub extension_chance: f32,
}

impl AnimalsPathConfig {
    /// AnimalsPathConfig's canonical constructor.
    pub fn new(
        animal_type: String,
        count: i32,
        chance: f32,
        points: Vec<i32>,
        radius: Vec<i32>,
        extension: Vec<i32>,
        extension_chance: f32,
    ) -> Self {
        AnimalsPathConfig {
            animal_type,
            count,
            chance,
            points,
            radius,
            extension,
            extension_chance,
        }
    }

    /// AnimalsPathConfig.getNameHash: `animalType.hashCode()` - java.lang.String.hashCode over the UTF-16
    /// code units.
    pub fn get_name_hash(&self) -> i32 {
        java::string_hash_code(&self.animal_type)
    }
}

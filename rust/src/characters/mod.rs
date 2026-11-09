//! Registry of available mascots, in the order the Python app lists them.

use crate::engine::Character;

pub mod cat;
pub mod crow;
pub mod dog;
pub mod goose;
pub mod horse;
pub mod monkey;
pub mod parrot;

/// Every mascot, in menu order.
pub fn all() -> &'static [&'static Character] {
    static ALL: &[&Character] = &[&monkey::CHARACTER, &goose::CHARACTER, &crow::CHARACTER, &parrot::CHARACTER, &dog::LABRADOR, &dog::COLLIE, &cat::CHARACTER, &horse::CHARACTER];
    ALL
}

pub fn get(name: &str) -> Option<&'static Character> {
    all().iter().copied().find(|c| c.name == name)
}

pub fn names() -> Vec<&'static str> {
    all().iter().map(|c| c.name).collect()
}

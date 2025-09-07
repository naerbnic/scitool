//! Types for reading the resource map file.

mod index;
mod index_entry;
mod location;
mod location_entry;
mod location_set;
mod type_locations;

pub(crate) use self::{location::ResourceLocation, location_set::ResourceLocationSet};

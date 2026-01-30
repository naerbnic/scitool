//! Provides low-level tools for editing and constructing SCI tools and games.
//!
//! This provides the low-level abstractions over reading and writing SCI
//! resources in a form that can be understood by the SCI engine, as well as the
//! structured data formats that are specific to SCI, such as the script/heap
//! resource formats.

#![deny(clippy::disallowed_types)] // Deny anyhow usage in this crate

pub mod ids;
pub mod resources;
pub mod script_loader;
pub mod utils;

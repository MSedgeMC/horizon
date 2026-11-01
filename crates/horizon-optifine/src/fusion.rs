//! Fusion's resource pack format (the Fusion Connected Textures mod): a `fusion` section in a
//! texture's `.mcmeta` (`texture`: connecting, random, continuous and scrolling textures, and
//! the emissive, render type and tinting options every type takes), the connection predicates
//! that say when a connecting texture joins a neighbour (`predicate`), the `connections` of a
//! `fusion:connecting` block model (`model`), and which tiles of a connecting texture's image a
//! face draws ([`layout`]).
//!
//! The layouts read the neighbour masks of [`crate::ctm::connect`], which OptiFine's connected
//! textures build too: the two formats share how a face finds its neighbours.
//!
//! Written from Fusion's public wiki (its texture type, model type and model predicate pages)
//! and the MIT-licensed layout templates of SuperMartijn642/FusionExamples, read off the images.
//! Fusion itself is all rights reserved: none of its code was read.

pub mod layout;

pub use layout::{Layout, Tiles};

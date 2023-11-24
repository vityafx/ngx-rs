//! The NGX-rs is a crate carefully wrapping the NVIDIA NGX library,
//! while also providing some abstractions in order to make the access
//! easier.
#![deny(missing_docs)]

#[allow(warnings)]
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(unused)]
#[allow(missing_docs)]
/// The raw bindings to the NVIDIA NGX library.
pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub mod error;
pub use error::*;
pub mod vk;
pub use vk::*;

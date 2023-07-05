//! NVIDIA NGX.

#![deny(missing_docs)]

use crate::bindings;
pub mod vk;

/// The result type used within the crate.
pub type Result<T = ()> = std::result::Result<T, bindings::NVSDK_NGX_Result>;

impl From<bindings::NVSDK_NGX_Result> for Result {
    fn from(value: bindings::NVSDK_NGX_Result) -> Self {
        match value {
            bindings::NVSDK_NGX_Result::NVSDK_NGX_Result_Success => Ok(()),
            e => Err(e),
        }
    }
}

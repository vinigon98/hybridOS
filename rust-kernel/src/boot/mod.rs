//! Boot Subsystem for HybridOS.
//!
//! Interacts with the Limine 64-bit bootloader protocol to query:
//! - Framebuffer (GOP video)
//! - Physical memory map
//! - Higher-Half Direct Map (HHDM) offset

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod limine;

pub use self::limine::{
    is_base_revision_supported,
    get_hhdm_offset,
    get_framebuffer,
    get_largest_usable_memory,
};

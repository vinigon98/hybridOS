//! Windows 11 Commercial Settings App (`ms-settings:`)
#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod system;
pub mod network;
pub mod personalization;
pub mod apps;
pub mod accounts;
pub mod time_gaming_access;
pub mod security_update;

pub use system::*;
pub use network::*;
pub use personalization::*;
pub use apps::*;
pub use accounts::*;
pub use time_gaming_access::*;
pub use security_update::*;

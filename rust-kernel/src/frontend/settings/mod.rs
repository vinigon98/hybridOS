//! Windows 11 Commercial Settings App (`ms-settings:`)
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

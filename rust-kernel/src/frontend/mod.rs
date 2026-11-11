//! HybridOS Windows 11 Original Commercial Frontend
//! Complete Fluent Design System, DWM, Taskbar, Start Menu, File Explorer,
//! Commercial Settings App, Task Manager, Terminal, Device Manager, and Core Tools.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod fluent;
pub mod dwm;
pub mod shell;
pub mod explorer;
pub mod settings;
pub mod apps;
pub mod win11_desktop;

pub use win11_desktop::Windows11Desktop;

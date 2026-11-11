//! Process and Handle Management subsystem.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::personality::ExecutionPersonality;
use crate::types::Handle;

#[derive(Debug, Clone)]
pub enum ObjectType {
    File { name: String, offset: u64 },
    Device { name: String },
    Process { pid: u32 },
    Thread { tid: u32, pid: u32, suspended: bool, suspend_count: u32 },
    Section { size: usize, protect: u32, name: Option<String> },
    Event { signaled: bool },
    Socket { id: usize },
    Epoll { id: usize },
}

#[derive(Debug, Clone)]
pub struct KernelObject {
    pub obj_type: ObjectType,
    pub access_mask: u32,
}

#[derive(Debug)]
pub struct HybridProcess {
    pub pid: u32,
    pub name: String,
    pub personality: ExecutionPersonality,
    pub image_base: u64,
    pub entry_point: u64,
    pub image_size: usize,
    pub handles: HashMap<Handle, Arc<Mutex<KernelObject>>>,
    pub next_handle: usize,
    pub is_terminated: bool,
    pub exit_code: i32,
}

impl HybridProcess {
    pub fn new(pid: u32, name: &str, personality: ExecutionPersonality) -> Self {
        let mut proc = Self {
            pid,
            name: name.to_string(),
            personality,
            image_base: 0,
            entry_point: 0,
            image_size: 0,
            handles: HashMap::new(),
            next_handle: 3,
            is_terminated: false,
            exit_code: 0,
        };

        // Standard stream descriptors: 0 = stdin, 1 = stdout, 2 = stderr
        proc.handles.insert(Handle::STDIN, Arc::new(Mutex::new(KernelObject {
            obj_type: ObjectType::Device { name: "console_in".into() },
            access_mask: 0x1,
        })));
        proc.handles.insert(Handle::STDOUT, Arc::new(Mutex::new(KernelObject {
            obj_type: ObjectType::Device { name: "console_out".into() },
            access_mask: 0x2,
        })));
        proc.handles.insert(Handle::STDERR, Arc::new(Mutex::new(KernelObject {
            obj_type: ObjectType::Device { name: "console_err".into() },
            access_mask: 0x2,
        })));

        proc
    }

    pub fn allocate_handle(&mut self, obj: KernelObject) -> Handle {
        let handle = Handle(self.next_handle);
        self.next_handle += 1;
        self.handles.insert(handle, Arc::new(Mutex::new(obj)));
        handle
    }

    pub fn get_object(&self, handle: Handle) -> Option<Arc<Mutex<KernelObject>>> {
        self.handles.get(&handle).cloned()
    }

    pub fn close_handle(&mut self, handle: Handle) -> bool {
        self.handles.remove(&handle).is_some()
    }
}

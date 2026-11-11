//! Limine Boot Protocol Interface (Protocol v2 / v3).
//!
//! Provides bare-metal requests for:
//! - Base Revision handshake
//! - Higher-Half Direct Map (HHDM)
//! - Framebuffer Video Mode
//! - Physical Memory Map



// Markers for Limine request scanning
#[used]
#[link_section = ".limine_requests_start_marker"]
static REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8f4b39de7d1ae, 0xfab91a6940fcb9cf,
    0x785c6ed015d3e316, 0x181e920a7852b9d9,
];

#[used]
#[link_section = ".limine_requests_end_marker"]
static REQUESTS_END_MARKER: [u64; 2] = [
    0xadc0e0531bb10d03, 0x9572709f31764c62,
];

// Base revision request
#[used]
#[link_section = ".limine_requests"]
pub static BASE_REVISION: [u64; 3] = [
    0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 3,
];

/// Returns true if Limine recognized and accepted the requested base revision.
pub fn is_base_revision_supported() -> bool {
    unsafe { core::ptr::read_volatile(&BASE_REVISION[2]) == 0 }
}

pub const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

// --- HHDM (Higher Half Direct Map) Request ---

#[repr(C)]
pub struct LimineHhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

#[repr(C)]
pub struct LimineHhdmRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *mut LimineHhdmResponse,
}

unsafe impl Sync for LimineHhdmRequest {}

#[used]
#[link_section = ".limine_requests"]
pub static mut HHDM_REQUEST: LimineHhdmRequest = LimineHhdmRequest {
    id: [LIMINE_COMMON_MAGIC[0], LIMINE_COMMON_MAGIC[1], 0x48dcf1cb8ad2b852, 0x63984e959a98244b],
    revision: 0,
    response: core::ptr::null_mut(),
};

// --- Framebuffer Request ---

#[repr(C)]
pub struct LimineFramebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    pub unused: [u8; 7],
    pub edid_size: u64,
    pub edid: *mut u8,
}

#[repr(C)]
pub struct LimineFramebufferResponse {
    pub revision: u64,
    pub framebuffer_count: u64,
    pub framebuffers: *mut *mut LimineFramebuffer,
}

#[repr(C)]
pub struct LimineFramebufferRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *mut LimineFramebufferResponse,
}

unsafe impl Sync for LimineFramebufferRequest {}

#[used]
#[link_section = ".limine_requests"]
pub static mut FRAMEBUFFER_REQUEST: LimineFramebufferRequest = LimineFramebufferRequest {
    id: [LIMINE_COMMON_MAGIC[0], LIMINE_COMMON_MAGIC[1], 0x9d5827dcd881dd75, 0xa3148604f6fab11b],
    revision: 0,
    response: core::ptr::null_mut(),
};

// --- Memory Map Request ---

pub const LIMINE_MEMMAP_USABLE: u64 = 0;
pub const LIMINE_MEMMAP_RESERVED: u64 = 1;
pub const LIMINE_MEMMAP_ACPI_RECLAIMABLE: u64 = 2;
pub const LIMINE_MEMMAP_ACPI_NVS: u64 = 3;
pub const LIMINE_MEMMAP_BAD_MEMORY: u64 = 4;
pub const LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE: u64 = 5;
pub const LIMINE_MEMMAP_EXECUTABLE_AND_MODULES: u64 = 6;
pub const LIMINE_MEMMAP_FRAMEBUFFER: u64 = 7;

#[repr(C)]
pub struct LimineMemmapEntry {
    pub base: u64,
    pub length: u64,
    pub typ: u64,
}

#[repr(C)]
pub struct LimineMemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: *mut *mut LimineMemmapEntry,
}

#[repr(C)]
pub struct LimineMemmapRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *mut LimineMemmapResponse,
}

unsafe impl Sync for LimineMemmapRequest {}

#[used]
#[link_section = ".limine_requests"]
pub static mut MEMMAP_REQUEST: LimineMemmapRequest = LimineMemmapRequest {
    id: [LIMINE_COMMON_MAGIC[0], LIMINE_COMMON_MAGIC[1], 0x67cf3d9d378a806f, 0xe304acdfc50c3c62],
    revision: 0,
    response: core::ptr::null_mut(),
};

/// Retrieves the HHDM (Higher-Half Direct Map) offset supplied by Limine.
pub fn get_hhdm_offset() -> u64 {
    unsafe {
        if !HHDM_REQUEST.response.is_null() {
            (*HHDM_REQUEST.response).offset
        } else {
            0xFFFF_8000_0000_0000
        }
    }
}

/// Retrieves primary framebuffer parameters if provided by the bootloader.
pub fn get_framebuffer() -> Option<(&'static mut [u8], u64, u64, u64)> {
    unsafe {
        if FRAMEBUFFER_REQUEST.response.is_null() {
            return None;
        }
        let resp = &*FRAMEBUFFER_REQUEST.response;
        if resp.framebuffer_count == 0 || resp.framebuffers.is_null() {
            return None;
        }
        let fb_ptr = *resp.framebuffers;
        if fb_ptr.is_null() {
            return None;
        }
        let fb = &*fb_ptr;
        let size = (fb.pitch * fb.height) as usize;
        let slice = core::slice::from_raw_parts_mut(fb.address, size);
        Some((slice, fb.width, fb.height, fb.pitch))
    }
}

/// Iterates over physical memory regions, returning (usable_base, usable_size).
pub fn get_largest_usable_memory() -> (u64, u64) {
    unsafe {
        if MEMMAP_REQUEST.response.is_null() {
            // Fallback: 128 MB at 2 MB
            return (0x0020_0000, 128 * 1024 * 1024);
        }
        let resp = &*MEMMAP_REQUEST.response;
        let mut best_base = 0u64;
        let mut best_length = 0u64;

        let entries = core::slice::from_raw_parts(resp.entries, resp.entry_count as usize);
        for &entry_ptr in entries {
            if entry_ptr.is_null() {
                continue;
            }
            let entry = &*entry_ptr;
            if entry.typ == LIMINE_MEMMAP_USABLE && entry.length > best_length {
                best_base = entry.base;
                best_length = entry.length;
            }
        }

        if best_length == 0 {
            (0x0020_0000, 128 * 1024 * 1024)
        } else {
            (best_base, best_length)
        }
    }
}

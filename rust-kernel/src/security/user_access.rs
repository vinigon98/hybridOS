//! Safe user-space memory access and hardware page-table walk validation.
//!
//! Enforces hardware memory boundaries for System V AMD64 and Microsoft x64:
//! - Canonical user-space boundary: addresses must be strictly < 0x0000_8000_0000_0000
//! - Non-null pointers (addresses starting at 0x0 are rejected)
//! - Active Hardware PML4 Walk: Verifies that every 4KiB page in the requested range
//!   has PRESENT and USER bits set, and WRITABLE when write access is requested.
//! - Prevents Ring 0 Kernel Page Faults (#PF) caused by invalid or unmapped user pointers.

use alloc::vec::Vec;
use core::mem::size_of;

/// Upper limit of 48-bit canonical user-space address space.
pub const USER_SPACE_LIMIT: usize = 0x0000_8000_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAccessError {
    NullPointer,
    AddressOutOfRange,
    ArithmeticOverflow,
    MisalignedPointer,
    UnmappedMemory,
    WritePermissionDenied,
}

/// Walks the active hardware page tables (CR3) to verify that `[ptr, ptr + len)`
/// is strictly resident, user-accessible, and optionally writable.
pub fn validate_user_memory(ptr: usize, len: usize, write: bool) -> Result<(), UserAccessError> {
    if ptr == 0 {
        return Err(UserAccessError::NullPointer);
    }
    let end = ptr.checked_add(len).ok_or(UserAccessError::ArithmeticOverflow)?;
    if end > USER_SPACE_LIMIT {
        return Err(UserAccessError::AddressOutOfRange);
    }

    let hhdm_offset = crate::boot::get_hhdm_offset();
    let cr3 = (unsafe { crate::arch::msr::read_cr3() }) & 0x000F_FFFF_FFFF_F000;
    if cr3 == 0 {
        return Err(UserAccessError::UnmappedMemory);
    }

    let pml4_base = (cr3 + hhdm_offset) as *const u64;

    // Page-aligned boundaries
    let start_page = ptr & !0xFFF;
    let end_page = (end + 0xFFF) & !0xFFF;

    let mut curr = start_page;
    while curr < end_page {
        let p4_idx = (curr >> 39) & 0x1FF;
        let p3_idx = (curr >> 30) & 0x1FF;
        let p2_idx = (curr >> 21) & 0x1FF;
        let p1_idx = (curr >> 12) & 0x1FF;

        unsafe {
            let p4_entry = *pml4_base.add(p4_idx);
            // Must be Present (bit 0) and User-accessible (bit 2)
            if (p4_entry & 1 == 0) || (p4_entry & (1 << 2) == 0) {
                return Err(UserAccessError::UnmappedMemory);
            }
            if write && (p4_entry & (1 << 1) == 0) {
                return Err(UserAccessError::WritePermissionDenied);
            }

            let pdpt_base = ((p4_entry & 0x000F_FFFF_FFFF_F000) + hhdm_offset) as *const u64;
            let p3_entry = *pdpt_base.add(p3_idx);
            if (p3_entry & 1 == 0) || (p3_entry & (1 << 2) == 0) {
                return Err(UserAccessError::UnmappedMemory);
            }
            if write && (p3_entry & (1 << 1) == 0) {
                return Err(UserAccessError::WritePermissionDenied);
            }

            // Check for 1GB Huge Page
            if p3_entry & (1 << 7) != 0 {
                curr = (curr + 0x4000_0000) & !0x3FFF_FFFF;
                continue;
            }

            let pd_base = ((p3_entry & 0x000F_FFFF_FFFF_F000) + hhdm_offset) as *const u64;
            let p2_entry = *pd_base.add(p2_idx);
            if (p2_entry & 1 == 0) || (p2_entry & (1 << 2) == 0) {
                return Err(UserAccessError::UnmappedMemory);
            }
            if write && (p2_entry & (1 << 1) == 0) {
                return Err(UserAccessError::WritePermissionDenied);
            }

            // Check for 2MB Large Page
            if p2_entry & (1 << 7) != 0 {
                curr = (curr + 0x20_0000) & !0x1F_FFFF;
                continue;
            }

            let pt_base = ((p2_entry & 0x000F_FFFF_FFFF_F000) + hhdm_offset) as *const u64;
            let p1_entry = *pt_base.add(p1_idx);
            if (p1_entry & 1 == 0) || (p1_entry & (1 << 2) == 0) {
                return Err(UserAccessError::UnmappedMemory);
            }
            if write && (p1_entry & (1 << 1) == 0) {
                return Err(UserAccessError::WritePermissionDenied);
            }
        }

        curr += 4096;
    }

    Ok(())
}

/// Safely copies a single value `T` from user space into kernel space after MMU verification.
pub unsafe fn copy_from_user<T: Copy>(user_ptr: *const T) -> Result<T, UserAccessError> {
    let addr = user_ptr as usize;
    let size = size_of::<T>();
    validate_user_memory(addr, size, false)?;
    if addr % core::mem::align_of::<T>() != 0 {
        return Err(UserAccessError::MisalignedPointer);
    }
    Ok(core::ptr::read_volatile(user_ptr))
}

/// Safely copies a single value `T` from kernel space into user space after MMU verification.
pub unsafe fn copy_to_user<T: Copy>(user_ptr: *mut T, src: &T) -> Result<(), UserAccessError> {
    let addr = user_ptr as usize;
    let size = size_of::<T>();
    validate_user_memory(addr, size, true)?;
    if addr % core::mem::align_of::<T>() != 0 {
        return Err(UserAccessError::MisalignedPointer);
    }
    core::ptr::write_volatile(user_ptr, *src);
    Ok(())
}

/// Safely copies a slice of bytes from user space into a kernel-allocated `Vec<u8>`.
pub unsafe fn copy_from_user_slice(user_ptr: *const u8, len: usize) -> Result<Vec<u8>, UserAccessError> {
    let addr = user_ptr as usize;
    validate_user_memory(addr, len, false)?;
    let mut buffer = Vec::with_capacity(len);
    buffer.set_len(len);
    core::ptr::copy_nonoverlapping(user_ptr, buffer.as_mut_ptr(), len);
    Ok(buffer)
}

/// Safely copies a kernel slice of bytes into user-space memory.
pub unsafe fn copy_to_user_slice(user_ptr: *mut u8, src: &[u8]) -> Result<(), UserAccessError> {
    let addr = user_ptr as usize;
    validate_user_memory(addr, src.len(), true)?;
    core::ptr::copy_nonoverlapping(src.as_ptr(), user_ptr, src.len());
    Ok(())
}

/// Safely reads a null-terminated UTF-8 / ASCII string from user space (up to `max_len` bytes).
pub unsafe fn copy_str_from_user(user_ptr: *const u8, max_len: usize) -> Result<alloc::string::String, UserAccessError> {
    let addr = user_ptr as usize;
    validate_user_memory(addr, 1, false)?;

    let mut bytes = Vec::new();
    let mut curr = user_ptr;
    for _ in 0..max_len {
        let curr_addr = curr as usize;
        if curr_addr >= USER_SPACE_LIMIT {
            return Err(UserAccessError::AddressOutOfRange);
        }
        // Verify byte's page
        if curr_addr & 0xFFF == 0 {
            validate_user_memory(curr_addr, 1, false)?;
        }
        let b = core::ptr::read_volatile(curr);
        if b == 0 {
            break;
        }
        bytes.push(b);
        curr = curr.add(1);
    }

    alloc::string::String::from_utf8(bytes).map_err(|_| UserAccessError::AddressOutOfRange)
}

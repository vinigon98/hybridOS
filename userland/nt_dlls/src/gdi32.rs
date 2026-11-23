//! gdi32.dll: Graphics Device Interface Library for HybridOS Ring 3.

use libhybrid_sys::windows::*;
use crate::user32::{HDC, HBRUSH, RECT};

pub type HBITMAP = usize;
pub type HGDIOBJ = usize;
pub type COLORREF = u32;

pub const SRCCOPY: u32 = 0x00CC0020;
pub const DIB_RGB_COLORS: u32 = 0;

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub bi_size: u32,
    pub bi_width: i32,
    pub bi_height: i32,
    pub bi_planes: u16,
    pub bi_bit_count: u16,
    pub bi_compression: u32,
    pub bi_size_image: u32,
    pub bi_x_pels_per_meter: i32,
    pub bi_y_pels_per_meter: i32,
    pub bi_clr_used: u32,
    pub bi_clr_important: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmi_header: BITMAPINFOHEADER,
    pub bmi_colors: [u32; 1],
}

#[no_mangle]
pub unsafe extern "win64" fn CreateCompatibleDC(h_dc: HDC) -> HDC {
    let res = nt_syscall1(SYS_NT_GDI_CREATE_COMPATIBLE_DC, h_dc as u64);
    if res != 0 { res as HDC } else { h_dc + 1 }
}

#[no_mangle]
pub unsafe extern "win64" fn DeleteDC(h_dc: HDC) -> i32 {
    let res = nt_syscall1(SYS_NT_GDI_DELETE_DC, h_dc as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateBitmap(
    n_width: i32,
    n_height: i32,
    n_planes: u32,
    n_bit_count: u32,
    lp_bits: *const u8,
) -> HBITMAP {
    let res = nt_syscall6(
        SYS_NT_GDI_CREATE_BITMAP,
        n_width as u64,
        n_height as u64,
        n_planes as u64,
        n_bit_count as u64,
        lp_bits as u64,
        0,
    );
    res as HBITMAP
}

#[no_mangle]
pub unsafe extern "win64" fn CreateCompatibleBitmap(
    h_dc: HDC,
    cx: i32,
    cy: i32,
) -> HBITMAP {
    let res = nt_syscall3(SYS_NT_GDI_CREATE_COMPATIBLE_BITMAP, h_dc as u64, cx as u64, cy as u64);
    res as HBITMAP
}

#[no_mangle]
pub unsafe extern "win64" fn SelectObject(h_dc: HDC, h_obj: HGDIOBJ) -> HGDIOBJ {
    let res = nt_syscall2(SYS_NT_GDI_SELECT_BITMAP, h_dc as u64, h_obj as u64);
    res as HGDIOBJ
}

#[no_mangle]
pub unsafe extern "win64" fn BitBlt(
    h_dc_dest: HDC,
    x_dest: i32,
    y_dest: i32,
    w: i32,
    h: i32,
    h_dc_src: HDC,
    x_src: i32,
    y_src: i32,
    rop: u32,
) -> i32 {
    let _ = (h_dc_src, x_src, y_src, rop);
    let res = nt_syscall6(
        SYS_NT_GDI_BIT_BLT,
        h_dc_dest as u64,
        x_dest as u64,
        y_dest as u64,
        w as u64,
        h as u64,
        rop as u64,
    );
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn FillRect(h_dc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32 {
    let res = nt_syscall3(SYS_NT_GDI_FILL_RECT, h_dc as u64, lprc as u64, hbr as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn Rectangle(
    h_dc: HDC,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> i32 {
    let res = nt_syscall6(
        SYS_NT_GDI_RECTANGLE,
        h_dc as u64,
        left as u64,
        top as u64,
        right as u64,
        bottom as u64,
        0,
    );
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn SetPixel(h_dc: HDC, x: i32, y: i32, color: COLORREF) -> COLORREF {
    let res = nt_syscall4(SYS_NT_GDI_SET_PIXEL, h_dc as u64, x as u64, y as u64, color as u64);
    res as COLORREF
}

#[no_mangle]
pub unsafe extern "win64" fn GetPixel(h_dc: HDC, x: i32, y: i32) -> COLORREF {
    let res = nt_syscall3(SYS_NT_GDI_GET_PIXEL, h_dc as u64, x as u64, y as u64);
    res as COLORREF
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSolidBrush(color: COLORREF) -> HBRUSH {
    let res = nt_syscall1(SYS_NT_GDI_CREATE_SOLID_BRUSH, color as u64);
    res as HBRUSH
}

#[no_mangle]
pub unsafe extern "win64" fn LineTo(h_dc: HDC, x: i32, y: i32) -> i32 {
    let res = nt_syscall3(SYS_NT_GDI_LINE_TO, h_dc as u64, x as u64, y as u64);
    if res == 0 { 1 } else { 0 }
}

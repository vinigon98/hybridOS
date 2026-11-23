//! user32.dll: Windows User Subsystem Library for HybridOS Ring 3.

#![no_std]

use libhybrid_sys::windows::*;

pub type HWND = usize;
pub type HDC = usize;
pub type HMENU = usize;
pub type HINSTANCE = usize;
pub type HICON = usize;
pub type HCURSOR = usize;
pub type HBRUSH = usize;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;

pub type WNDPROC = Option<unsafe extern "win64" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub w_param: WPARAM,
    pub l_param: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cb_size: u32,
    pub style: u32,
    pub lpfn_wnd_proc: WNDPROC,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: HINSTANCE,
    pub h_icon: HICON,
    pub h_cursor: HCURSOR,
    pub hbr_background: HBRUSH,
    pub lpsz_menu_name: *const u16,
    pub lpsz_class_name: *const u16,
    pub h_icon_sm: HICON,
}

pub const WM_NULL: u32        = 0x0000;
pub const WM_CREATE: u32      = 0x0001;
pub const WM_DESTROY: u32     = 0x0002;
pub const WM_MOVE: u32        = 0x0003;
pub const WM_SIZE: u32        = 0x0005;
pub const WM_ACTIVATE: u32    = 0x0006;
pub const WM_PAINT: u32       = 0x000F;
pub const WM_CLOSE: u32       = 0x0010;
pub const WM_QUIT: u32        = 0x0012;
pub const WM_KEYDOWN: u32     = 0x0100;
pub const WM_KEYUP: u32       = 0x0101;
pub const WM_MOUSEMOVE: u32   = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32   = 0x0202;

#[no_mangle]
pub unsafe extern "win64" fn CreateWindowExA(
    dw_ex_style: u32,
    lp_class_name: *const u8,
    lp_window_name: *const u8,
    dw_style: u32,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    h_wnd_parent: HWND,
    h_menu: HMENU,
    h_instance: HINSTANCE,
    lp_param: usize,
) -> HWND {
    let _ = (h_menu, h_instance, lp_param);
    let res = nt_syscall6(
        SYS_NT_USER_CREATE_WINDOW_EX,
        dw_ex_style as u64,
        lp_class_name as u64,
        lp_window_name as u64,
        dw_style as u64,
        ((x as u64) << 32) | (y as u32 as u64),
        ((n_width as u64) << 32) | (n_height as u32 as u64),
    );
    if res != 0 {
        res as HWND
    } else {
        h_wnd_parent + 1
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWindowExW(
    dw_ex_style: u32,
    lp_class_name: *const u16,
    lp_window_name: *const u16,
    dw_style: u32,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    h_wnd_parent: HWND,
    h_menu: HMENU,
    h_instance: HINSTANCE,
    lp_param: usize,
) -> HWND {
    let _ = (h_menu, h_instance, lp_param);
    let res = nt_syscall6(
        SYS_NT_USER_CREATE_WINDOW_EX,
        dw_ex_style as u64,
        lp_class_name as u64,
        lp_window_name as u64,
        dw_style as u64,
        ((x as u64) << 32) | (y as u32 as u64),
        ((n_width as u64) << 32) | (n_height as u32 as u64),
    );
    if res != 0 {
        res as HWND
    } else {
        h_wnd_parent + 1
    }
}

#[no_mangle]
pub unsafe extern "win64" fn DestroyWindow(h_wnd: HWND) -> i32 {
    let res = nt_syscall1(SYS_NT_USER_DESTROY_WINDOW, h_wnd as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn ShowWindow(h_wnd: HWND, n_cmd_show: i32) -> i32 {
    let res = nt_syscall2(SYS_NT_USER_SHOW_WINDOW, h_wnd as u64, n_cmd_show as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowPos(
    h_wnd: HWND,
    h_wnd_insert_after: HWND,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    u_flags: u32,
) -> i32 {
    let res = nt_syscall6(
        SYS_NT_USER_SET_WINDOW_POS,
        h_wnd as u64,
        h_wnd_insert_after as u64,
        x as u64,
        y as u64,
        cx as u64,
        cy as u64,
    );
    let _ = u_flags;
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessageA(
    lp_msg: *mut MSG,
    h_wnd: HWND,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
) -> i32 {
    let res = nt_syscall4(
        SYS_NT_USER_GET_MESSAGE,
        lp_msg as u64,
        h_wnd as u64,
        w_msg_filter_min as u64,
        w_msg_filter_max as u64,
    );
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessageW(
    lp_msg: *mut MSG,
    h_wnd: HWND,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
) -> i32 {
    GetMessageA(lp_msg, h_wnd, w_msg_filter_min, w_msg_filter_max)
}

#[no_mangle]
pub unsafe extern "win64" fn PeekMessageA(
    lp_msg: *mut MSG,
    h_wnd: HWND,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
    w_remove_msg: u32,
) -> i32 {
    let res = nt_syscall6(
        SYS_NT_USER_PEEK_MESSAGE,
        lp_msg as u64,
        h_wnd as u64,
        w_msg_filter_min as u64,
        w_msg_filter_max as u64,
        w_remove_msg as u64,
        0,
    );
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn PeekMessageW(
    lp_msg: *mut MSG,
    h_wnd: HWND,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
    w_remove_msg: u32,
) -> i32 {
    PeekMessageA(lp_msg, h_wnd, w_msg_filter_min, w_msg_filter_max, w_remove_msg)
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageA(
    h_wnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> i32 {
    let res = nt_syscall4(SYS_NT_USER_POST_MESSAGE, h_wnd as u64, msg as u64, w_param as u64, l_param as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageW(
    h_wnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> i32 {
    PostMessageA(h_wnd, msg, w_param, l_param)
}

#[no_mangle]
pub unsafe extern "win64" fn DispatchMessageA(lp_msg: *const MSG) -> LRESULT {
    let res = nt_syscall1(SYS_NT_USER_DISPATCH_MESSAGE, lp_msg as u64);
    res as LRESULT
}

#[no_mangle]
pub unsafe extern "win64" fn DispatchMessageW(lp_msg: *const MSG) -> LRESULT {
    DispatchMessageA(lp_msg)
}

#[no_mangle]
pub unsafe extern "win64" fn DefWindowProcA(
    h_wnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    let res = nt_syscall4(SYS_NT_USER_DEF_WINDOW_PROC, h_wnd as u64, msg as u64, w_param as u64, l_param as u64);
    res as LRESULT
}

#[no_mangle]
pub unsafe extern "win64" fn DefWindowProcW(
    h_wnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    DefWindowProcA(h_wnd, msg, w_param, l_param)
}

#[no_mangle]
pub unsafe extern "win64" fn GetDC(h_wnd: HWND) -> HDC {
    let res = nt_syscall1(SYS_NT_USER_GET_DC, h_wnd as u64);
    if res != 0 { res as HDC } else { 1 }
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseDC(h_wnd: HWND, h_dc: HDC) -> i32 {
    let res = nt_syscall2(SYS_NT_USER_RELEASE_DC, h_wnd as u64, h_dc as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetClientRect(h_wnd: HWND, lp_rect: *mut RECT) -> i32 {
    let res = nt_syscall2(SYS_NT_USER_GET_CLIENT_RECT, h_wnd as u64, lp_rect as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowRect(h_wnd: HWND, lp_rect: *mut RECT) -> i32 {
    let res = nt_syscall2(SYS_NT_USER_GET_WINDOW_RECT, h_wnd as u64, lp_rect as u64);
    if res == 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn InvalidateRect(
    h_wnd: HWND,
    lp_rect: *const RECT,
    b_erase: i32,
) -> i32 {
    let res = nt_syscall3(SYS_NT_USER_INVALIDATE_RECT, h_wnd as u64, lp_rect as u64, b_erase as u64);
    if res == 0 { 1 } else { 0 }
}

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod vdm;
pub mod hardware;
pub mod ata;
pub mod e1000;
pub mod gop;
pub mod mouse;
pub mod desktop;
pub mod gfx2d;
pub mod hda;

use core::sync::atomic::{AtomicBool, Ordering};

const KBD_BUF_CAP: usize = 256;

/// High-performance static circular ring buffer.
/// Safe for IRQ contexts: Zero heap allocation, O(1) push and O(1) pop.
struct KeyboardRingBuffer {
    data: [char; KBD_BUF_CAP],
    head: usize,
    tail: usize,
    count: usize,
    overflow_count: u32,
}

impl KeyboardRingBuffer {
    const fn new() -> Self {
        Self {
            data: ['\0'; KBD_BUF_CAP],
            head: 0,
            tail: 0,
            count: 0,
            overflow_count: 0,
        }
    }

    fn push(&mut self, c: char) -> bool {
        if self.count < KBD_BUF_CAP {
            self.data[self.tail] = c;
            self.tail = (self.tail + 1) & (KBD_BUF_CAP - 1);
            self.count += 1;
            true
        } else {
            self.overflow_count = self.overflow_count.saturating_add(1);
            false
        }
    }

    fn pop(&mut self) -> Option<char> {
        if self.count > 0 {
            let c = self.data[self.head];
            self.head = (self.head + 1) & (KBD_BUF_CAP - 1);
            self.count -= 1;
            Some(c)
        } else {
            None
        }
    }
}

static KEYBOARD_BUFFER: crate::sync::SpinLock<KeyboardRingBuffer> =
    crate::sync::SpinLock::new(KeyboardRingBuffer::new());
static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);

/// Enqueues an ASCII character from the PS/2 keyboard IRQ handler (O(1), zero allocation).
pub fn enqueue_keyboard_char(c: char) {
    let mut buf = KEYBOARD_BUFFER.lock();
    buf.push(c);
}

/// Pops the next available ASCII character from the keyboard input buffer (O(1)).
pub fn pop_keyboard_char() -> Option<char> {
    let mut buf = KEYBOARD_BUFFER.lock();
    buf.pop()
}

/// Returns dropped key count due to buffer overflow.
#[allow(dead_code)]
pub fn get_keyboard_overflow_count() -> u32 {
    let buf = KEYBOARD_BUFFER.lock();
    buf.overflow_count
}

/// Polls the i8042 PS/2 controller (port 0x64/0x60) and dispatches mouse packets (AUX bit 0x20) vs keyboard scancodes.
/// Drains up to 64 bytes to prevent controller FIFO overrun and discards parity/timeout errors.
pub fn poll_ps2_controller() -> bool {
    let mut had_event = false;
    for _ in 0..64 {
        let status = unsafe { crate::arch::msr::inb(0x64) };
        if (status & 0x01) == 0 {
            break;
        }
        // Bit 7 (Parity error) ou Bit 6 (Receive timeout) indicam corrupcao de sinal
        if (status & 0xC0) != 0 {
            let _ = unsafe { crate::arch::msr::inb(0x60) };
            continue;
        }
        let data = unsafe { crate::arch::msr::inb(0x60) };
        if (status & 0x20) != 0 {
            mouse::handle_mouse_packet(data);
            had_event = true;
        } else {
            handle_keyboard_irq(data);
            had_event = true;
        }
    }
    had_event
}

/// Handles keyboard scancode received from port 0x60 on IRQ 1.
pub fn handle_keyboard_irq(scancode: u8) {
    match scancode {
        0x2A | 0x36 => {
            SHIFT_PRESSED.store(true, Ordering::SeqCst);
            return;
        }
        0xAA | 0xB6 => {
            SHIFT_PRESSED.store(false, Ordering::SeqCst);
            return;
        }
        _ => {}
    }

    // Ignore key release events (bit 7 set)
    if scancode & 0x80 != 0 {
        return;
    }

    let shift = SHIFT_PRESSED.load(Ordering::SeqCst);

    let ascii = match scancode {
        0x02 => Some(if shift { '!' } else { '1' }),
        0x03 => Some(if shift { '@' } else { '2' }),
        0x04 => Some(if shift { '#' } else { '3' }),
        0x05 => Some(if shift { '$' } else { '4' }),
        0x06 => Some(if shift { '%' } else { '5' }),
        0x07 => Some(if shift { '^' } else { '6' }),
        0x08 => Some(if shift { '&' } else { '7' }),
        0x09 => Some(if shift { '*' } else { '8' }),
        0x0A => Some(if shift { '(' } else { '9' }),
        0x0B => Some(if shift { ')' } else { '0' }),
        0x0C => Some(if shift { '_' } else { '-' }),
        0x0D => Some(if shift { '+' } else { '=' }),
        0x0E => Some('\x08'), // Backspace
        0x0F => Some('\t'),   // Tab
        0x10 => Some(if shift { 'Q' } else { 'q' }),
        0x11 => Some(if shift { 'W' } else { 'w' }),
        0x12 => Some(if shift { 'E' } else { 'e' }),
        0x13 => Some(if shift { 'R' } else { 'r' }),
        0x14 => Some(if shift { 'T' } else { 't' }),
        0x15 => Some(if shift { 'Y' } else { 'y' }),
        0x16 => Some(if shift { 'U' } else { 'u' }),
        0x17 => Some(if shift { 'I' } else { 'i' }),
        0x18 => Some(if shift { 'O' } else { 'o' }),
        0x19 => Some(if shift { 'P' } else { 'p' }),
        0x1A => Some(if shift { '{' } else { '[' }),
        0x1B => Some(if shift { '}' } else { ']' }),
        0x1C => Some('\n'),   // Enter
        0x1E => Some(if shift { 'A' } else { 'a' }),
        0x1F => Some(if shift { 'S' } else { 's' }),
        0x20 => Some(if shift { 'D' } else { 'd' }),
        0x21 => Some(if shift { 'F' } else { 'f' }),
        0x22 => Some(if shift { 'G' } else { 'g' }),
        0x23 => Some(if shift { 'H' } else { 'h' }),
        0x24 => Some(if shift { 'J' } else { 'j' }),
        0x25 => Some(if shift { 'K' } else { 'k' }),
        0x26 => Some(if shift { 'L' } else { 'l' }),
        0x27 => Some(if shift { ':' } else { ';' }),
        0x28 => Some(if shift { '"' } else { '\'' }),
        0x29 => Some(if shift { '~' } else { '`' }),
        0x2B => Some(if shift { '|' } else { '\\' }),
        0x2C => Some(if shift { 'Z' } else { 'z' }),
        0x2D => Some(if shift { 'X' } else { 'x' }),
        0x2E => Some(if shift { 'C' } else { 'c' }),
        0x2F => Some(if shift { 'V' } else { 'v' }),
        0x30 => Some(if shift { 'B' } else { 'b' }),
        0x31 => Some(if shift { 'N' } else { 'n' }),
        0x32 => Some(if shift { 'M' } else { 'm' }),
        0x33 => Some(if shift { '<' } else { ',' }),
        0x34 => Some(if shift { '>' } else { '.' }),
        0x35 => Some(if shift { '?' } else { '/' }),
        0x39 => Some(' '),    // Space
        _ => None,
    };

    if let Some(ch) = ascii {
        enqueue_keyboard_char(ch);
    }
}

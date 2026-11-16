//! PS/2 Mouse Driver (IRQ 12) with IntelliMouse 4-byte Wheel Support,
//! graceful 3-byte fallback, pointer ballistics acceleration, and non-blocking event queue.

use crate::sync::SpinLock;
use crate::arch::msr::{inb, outb};

/// Snapshot of the mouse state, maintained for both legacy callers and absolute positioning.
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub z: i32,                   // Accumulated scroll position
    pub scroll_delta: i32,        // Unconsumed scroll ticks since last poll
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub is_intellimouse: bool,
    pub packet_size: u8,          // 4 for IntelliMouse (with wheel), 3 for standard PS/2
    pub sensitivity: u32,         // Sensitivity percentage (100 = 1.0x)
    pub accel_enabled: bool,      // Ballistic acceleration flag
    packet_phase: u8,
    packet_buf: [u8; 4],
}

pub static MOUSE_STATE: SpinLock<MouseState> = SpinLock::new(MouseState {
    x: 640,
    y: 400,
    z: 0,
    scroll_delta: 0,
    left_button: false,
    right_button: false,
    middle_button: false,
    is_intellimouse: false,
    packet_size: 3,
    sensitivity: 100,
    accel_enabled: true,
    packet_phase: 0,
    packet_buf: [0; 4],
});

/// Granular mouse event stored in the event ring buffer to avoid dropped clicks or scrolls.
#[derive(Copy, Clone, Debug, Default)]
pub struct MouseEvent {
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}

const MOUSE_QUEUE_CAP: usize = 64;

pub struct MouseEventQueue {
    events: [MouseEvent; MOUSE_QUEUE_CAP],
    head: usize,
    tail: usize,
    count: usize,
    pub overflow_count: u32,
}

impl MouseEventQueue {
    pub const fn new() -> Self {
        Self {
            events: [MouseEvent {
                dx: 0,
                dy: 0,
                dz: 0,
                left_button: false,
                right_button: false,
                middle_button: false,
            }; MOUSE_QUEUE_CAP],
            head: 0,
            tail: 0,
            count: 0,
            overflow_count: 0,
        }
    }

    pub fn push(&mut self, ev: MouseEvent) -> bool {
        if self.count < MOUSE_QUEUE_CAP {
            self.events[self.tail] = ev;
            self.tail = (self.tail + 1) % MOUSE_QUEUE_CAP;
            self.count += 1;
            true
        } else {
            self.overflow_count = self.overflow_count.saturating_add(1);
            false
        }
    }

    pub fn pop(&mut self) -> Option<MouseEvent> {
        if self.count > 0 {
            let ev = self.events[self.head];
            self.head = (self.head + 1) % MOUSE_QUEUE_CAP;
            self.count -= 1;
            Some(ev)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

pub static MOUSE_EVENT_QUEUE: SpinLock<MouseEventQueue> = SpinLock::new(MouseEventQueue::new());

#[inline]
unsafe fn ps2_wait_write() {
    for _ in 0..10_000 {
        if (inb(0x64) & 0x02) == 0 {
            return;
        }
    }
}

#[inline]
unsafe fn ps2_wait_read() -> bool {
    for _ in 0..10_000 {
        if (inb(0x64) & 0x01) != 0 {
            return true;
        }
    }
    false
}

#[inline]
unsafe fn ps2_drain_output() {
    for _ in 0..32 {
        if (inb(0x64) & 0x01) == 0 {
            break;
        }
        let _ = inb(0x60);
    }
}

#[inline]
unsafe fn ps2_mouse_write(cmd: u8) -> bool {
    ps2_wait_write();
    outb(0x64, 0xD4);
    ps2_wait_write();
    outb(0x60, cmd);
    // Consome e valida o byte ACK (0xFA)
    if ps2_wait_read() {
        let ack = inb(0x60);
        ack == 0xFA
    } else {
        false
    }
}

#[inline]
unsafe fn ps2_mouse_read() -> Option<u8> {
    if ps2_wait_read() {
        Some(inb(0x60))
    } else {
        None
    }
}

/// Initializes the PS/2 mouse with IntelliMouse detection knock and fallback.
pub fn init_ps2_mouse() {
    let mut is_intellimouse = false;
    let mut packet_size = 3u8;

    unsafe {
        ps2_drain_output();

        // 1. Habilita a porta auxiliar (mouse) no controlador i8042
        ps2_wait_write();
        outb(0x64, 0xA8);

        // 2. Le o byte de configuracao do controlador e ativa IRQ12 (bit 1), clock ativo (limpa bit 5)
        ps2_wait_write();
        outb(0x64, 0x20);
        let mut status = if ps2_wait_read() { inb(0x60) } else { 0 };
        status = (status | 0x02) & !0x20;
        ps2_wait_write();
        outb(0x64, 0x60);
        ps2_wait_write();
        outb(0x60, status);

        // 3. Define configuracoes padrao (0xF6)
        ps2_mouse_write(0xF6);

        // 4. Sequencia de knock do IntelliMouse para ativar o 4º byte (Wheel):
        // Set sample rate 200 -> 100 -> 80
        ps2_mouse_write(0xF3);
        ps2_mouse_write(200);

        ps2_mouse_write(0xF3);
        ps2_mouse_write(100);

        ps2_mouse_write(0xF3);
        ps2_mouse_write(80);

        // 5. Pergunta o Device ID (0xF2)
        ps2_mouse_write(0xF2);
        if let Some(dev_id) = ps2_mouse_read() {
            if dev_id == 3 || dev_id == 4 {
                is_intellimouse = true;
                packet_size = 4;
                crate::serial_println!("[MOUSE] IntelliMouse PS/2 detected! ID=0x{:02X} (4-byte packet, wheel active)", dev_id);
            } else {
                crate::serial_println!("[MOUSE] Standard PS/2 mouse detected. ID=0x{:02X} (3-byte fallback)", dev_id);
            }
        } else {
            crate::serial_println!("[MOUSE] No response to ID query; fallback to standard 3-byte mode.");
        }

        // 6. Configura Sample Rate operacional ideal (100 Hz)
        ps2_mouse_write(0xF3);
        ps2_mouse_write(100);

        // 7. Configura resolucao para 8 counts/mm (0xE8, 0x03) para maxima precisao
        ps2_mouse_write(0xE8);
        ps2_mouse_write(0x03);

        // 8. Ativa envio de pacotes (0xF4)
        ps2_mouse_write(0xF4);
        ps2_drain_output();
    }

    let (max_w, max_h) = get_screen_bounds();
    let mut mouse = MOUSE_STATE.lock();
    mouse.x = max_w / 2;
    mouse.y = max_h / 2;
    mouse.z = 0;
    mouse.scroll_delta = 0;
    mouse.left_button = false;
    mouse.right_button = false;
    mouse.middle_button = false;
    mouse.is_intellimouse = is_intellimouse;
    mouse.packet_size = packet_size;
    mouse.packet_phase = 0;
    mouse.packet_buf = [0; 4];
    mouse.sensitivity = 100;
    mouse.accel_enabled = true;
}

fn get_screen_bounds() -> (i32, i32) {
    let con = crate::drivers::gop::GOP_CONSOLE.lock();
    if let Some(ref c) = *con {
        if c.width > 0 && c.height > 0 {
            return (c.width as i32, c.height as i32);
        }
    }
    (1280, 800)
}

/// Applies integer-only ballistic acceleration curve and sensitivity scaling.
#[inline]
fn apply_pointer_ballistics(dx: i32, dy: i32, sensitivity_percent: u32, accel_enabled: bool) -> (i32, i32) {
    if !accel_enabled {
        let sx = (dx * sensitivity_percent as i32) / 100;
        let sy = (dy * sensitivity_percent as i32) / 100;
        return (sx, sy);
    }

    let mag = dx.abs().max(dy.abs());
    
    // Curva balistica segmentada (multiplicadores inteiros com base 10):
    // <= 3: 1.0x (precisao fina pixel-a-pixel)
    // 4..8: 1.5x (movimento comodo de cursor)
    // 9..15: 2.0x (deslocamento rapido)
    // > 15: 2.8x (flick para travessia rapida de tela)
    let mult = if mag <= 3 {
        10
    } else if mag <= 8 {
        15
    } else if mag <= 15 {
        20
    } else {
        28
    };

    let ax = (dx * mult) / 10;
    let ay = (dy * mult) / 10;

    let sx = (ax * sensitivity_percent as i32) / 100;
    let sy = (ay * sensitivity_percent as i32) / 100;
    (sx, sy)
}

/// Handles incoming PS/2 mouse bytes with sync-alignment and 3/4-byte phase processing.
pub fn handle_mouse_packet(byte: u8) {
    let (max_w, max_h) = get_screen_bounds();
    let mut mouse = MOUSE_STATE.lock();
    let phase = mouse.packet_phase;
    let packet_size = mouse.packet_size;

    // Byte 0 do protocolo PS/2 tem obrigatoriamente o bit 3 (0x08) setado.
    // Se nao estiver setado em fase 0, houve dessincronizacao de fluxo: descarta.
    if phase == 0 && (byte & 0x08) == 0 {
        return;
    }

    mouse.packet_buf[phase as usize] = byte;
    let next_phase = (phase + 1) % packet_size;
    mouse.packet_phase = next_phase;

    if next_phase == 0 {
        let flags = mouse.packet_buf[0];
        let raw_x = mouse.packet_buf[1] as i32;
        let raw_y = mouse.packet_buf[2] as i32;

        // Ignora pacotes com overflow de hardware no eixo X ou Y
        if (flags & 0xC0) != 0 {
            return;
        }

        // Reconstrucao de sinal de 9 bits conforme especificacao PS/2
        let raw_dx = if (flags & 0x10) != 0 { raw_x - 256 } else { raw_x };
        let raw_dy = if (flags & 0x20) != 0 { raw_y - 256 } else { raw_y };

        // Extracao do eixo Z (scroll wheel) no modo 4-byte IntelliMouse
        let dz = if packet_size == 4 {
            let raw_z = (mouse.packet_buf[3] & 0x0F) as i8;
            if (raw_z & 0x08) != 0 {
                (raw_z | !0x0F) as i32
            } else {
                raw_z as i32
            }
        } else {
            0
        };

        // Aplica balistica de aceleracao e sensibilidade
        let (dx, dy) = apply_pointer_ballistics(
            raw_dx,
            raw_dy,
            mouse.sensitivity,
            mouse.accel_enabled,
        );

        let left = (flags & 0x01) != 0;
        let right = (flags & 0x02) != 0;
        let middle = (flags & 0x04) != 0;

        mouse.left_button = left;
        mouse.right_button = right;
        mouse.middle_button = middle;

        mouse.z += dz;
        mouse.scroll_delta += dz;

        let max_x = (max_w - 1).max(0);
        let max_y = (max_h - 1).max(0);
        mouse.x = (mouse.x + dx).clamp(0, max_x);
        mouse.y = (mouse.y - dy).clamp(0, max_y);

        // Enfileira evento na fila circular para garantir que cliques rapidos nao sejam perdidos
        let ev = MouseEvent {
            dx,
            dy,
            dz,
            left_button: left,
            right_button: right,
            middle_button: middle,
        };
        let _ = MOUSE_EVENT_QUEUE.lock().push(ev);
    }
}

/// Pops next event from mouse queue.
#[allow(dead_code)]
pub fn pop_mouse_event() -> Option<MouseEvent> {
    MOUSE_EVENT_QUEUE.lock().pop()
}

/// Atomically reads and clears accumulated scroll delta.
#[allow(dead_code)]
pub fn pop_scroll_delta() -> i32 {
    let mut mouse = MOUSE_STATE.lock();
    let d = mouse.scroll_delta;
    mouse.scroll_delta = 0;
    d
}

/// Sets mouse sensitivity percentage (10 to 500, default 100).
#[allow(dead_code)]
pub fn set_mouse_sensitivity(sensitivity_percent: u32) {
    let mut mouse = MOUSE_STATE.lock();
    mouse.sensitivity = sensitivity_percent.clamp(10, 500);
}

/// Enables or disables ballistic acceleration curve.
#[allow(dead_code)]
pub fn set_mouse_acceleration(enabled: bool) {
    let mut mouse = MOUSE_STATE.lock();
    mouse.accel_enabled = enabled;
}

/// Dynamically updates the hardware sample rate.
#[allow(dead_code)]
pub fn set_mouse_sample_rate(rate: u8) {
    unsafe {
        ps2_mouse_write(0xF3);
        ps2_mouse_write(rate);
    }
}

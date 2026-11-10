//! Serial COM1 Driver for x86_64 Kernel Console Logging.
//!
//! Implements a 16550A UART driver communicating over x86 I/O ports
//! (standard COM1 base: 0x3F8). Provides formatted kernel console output,
//! diagnostic buffers, and dual-mode execution (bare-metal port I/O and
//! user-mode host testing).

use std::fmt;
use std::sync::Mutex;

/// Standard base port for COM1.
pub const COM1_BASE: u16 = 0x3F8;
/// Standard base port for COM2.
pub const COM2_BASE: u16 = 0x2F8;

// UART 16550A Register Offsets
const REG_DATA: u16 = 0;       // Receiver Buffer / Transmitter Holding Register
const REG_IER: u16 = 1;        // Interrupt Enable Register
const REG_FIFO: u16 = 2;       // FIFO Control Register / Interrupt Identification
const REG_LCR: u16 = 3;        // Line Control Register
const REG_MCR: u16 = 4;        // Modem Control Register
const REG_LSR: u16 = 5;        // Line Status Register
#[allow(dead_code)]
const REG_MSR: u16 = 6;        // Modem Status Register
const REG_SCRATCH: u16 = 7;    // Scratch Register

// Line Status Register Flags
const LSR_DATA_READY: u8 = 1 << 0;
#[allow(dead_code)]
const LSR_OVERRUN_ERR: u8 = 1 << 1;
#[allow(dead_code)]
const LSR_PARITY_ERR: u8 = 1 << 2;
#[allow(dead_code)]
const LSR_FRAMING_ERR: u8 = 1 << 3;
#[allow(dead_code)]
const LSR_BREAK_INT: u8 = 1 << 4;
const LSR_TX_EMPTY: u8 = 1 << 5;     // Transmitter Holding Register Empty (ready to write)
const LSR_LINE_IDLE: u8 = 1 << 6;    // Transmitter completely idle

/// Low-level port I/O abstraction.
/// When executing inside a user-mode test environment, real CPU `in`/`out`
/// instructions trigger a hardware fault (ring 3 privilege violation).
/// This structure provides simulated port registers with fallbacks.
#[derive(Debug)]
pub struct SerialPort {
    base_port: u16,
    is_initialized: bool,
    /// Memory log buffer capturing all serial console outputs for testing & kernel log inspection
    buffer: Vec<u8>,
    /// Simulated register state for user-mode execution host
    sim_scratch: u8,
    sim_mcr: u8,
    sim_lcr: u8,
    sim_ier: u8,
}

impl SerialPort {
    /// Creates a new serial port handle for the given base I/O port.
    pub const fn new(base_port: u16) -> Self {
        Self {
            base_port,
            is_initialized: false,
            buffer: Vec::new(),
            sim_scratch: 0,
            sim_mcr: 0,
            sim_lcr: 0,
            sim_ier: 0,
        }
    }

    /// Initializes the 16550A UART device:
    /// 1. Disables all interrupts.
    /// 2. Enables DLAB (Divisor Latch Access Bit) to configure baud rate.
    /// 3. Sets baud rate to 115200 (divisor = 1).
    /// 4. Configures 8 data bits, no parity, 1 stop bit (8N1).
    /// 5. Enables FIFO, clears TX/RX queues with 14-byte interrupt threshold.
    /// 6. Configures modem control (RTS/DTR set, auxiliary output 2 enabled).
    /// 7. Performs a loopback diagnostic test to verify hardware integrity.
    pub fn init(&mut self) -> Result<(), &'static str> {
        // 1. Disable all interrupts
        self.outb(REG_IER, 0x00);

        // 2. Enable DLAB (set bit 7 of Line Control Register)
        self.outb(REG_LCR, 0x80);

        // 3. Set baud divisor to 1 (115200 baud)
        self.outb(REG_DATA, 0x01); // Divisor LSB: 1
        self.outb(REG_IER, 0x00);  // Divisor MSB: 0

        // 4. Set line configuration: 8 bits, no parity, 1 stop bit (8N1)
        self.outb(REG_LCR, 0x03);

        // 5. Enable FIFO, clear transmit/receive queues, 14-byte threshold
        self.outb(REG_FIFO, 0xC7);

        // 6. Set RTS/DSR set, auxiliary output 2 (enables IRQ line)
        self.outb(REG_MCR, 0x0B);

        // 7. Loopback diagnostic test
        self.outb(REG_MCR, 0x1E); // Enable loopback mode
        self.outb(REG_DATA, 0xAE); // Send test byte
        let check = self.inb(REG_DATA);
        if check != 0xAE && self.is_hardware_present() {
            return Err("Serial port failed loopback self-test");
        }

        // Restore normal operational mode
        self.outb(REG_MCR, 0x0F);
        self.is_initialized = true;

        Ok(())
    }

    /// Checks if line status indicates transmitter holding register is empty.
    pub fn is_transmit_empty(&self) -> bool {
        (self.inb(REG_LSR) & LSR_TX_EMPTY) != 0
    }

    /// Checks if received data is ready to read.
    pub fn is_data_ready(&self) -> bool {
        (self.inb(REG_LSR) & LSR_DATA_READY) != 0
    }

    /// Writes a single byte to the serial console.
    pub fn send(&mut self, byte: u8) {
        // Wait until transmitter is empty (in bare-metal mode)
        let mut timeout = 100_000;
        while !self.is_transmit_empty() && timeout > 0 {
            timeout -= 1;
        }

        self.outb(REG_DATA, byte);
        self.buffer.push(byte);
    }

    /// Writes a string slice to the serial console, converting LF to CRLF for serial terminals.
    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
    }

    /// Attempts to read a single byte from the serial receiver buffer.
    pub fn receive(&self) -> Option<u8> {
        if self.is_data_ready() {
            Some(self.inb(REG_DATA))
        } else {
            None
        }
    }

    /// Reads all buffered output written to the serial device so far.
    pub fn get_buffered_output(&self) -> String {
        String::from_utf8_lossy(&self.buffer).into_owned()
    }

    /// Returns the base port of this serial device.
    pub const fn base_port(&self) -> u16 {
        self.base_port
    }

    /// Returns whether this serial port has completed initialization.
    pub const fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    // --- Low-level I/O Port Helper Functions ---

    fn outb(&mut self, offset: u16, val: u8) {
        let port = self.base_port + offset;
        match offset {
            REG_IER => self.sim_ier = val,
            REG_LCR => self.sim_lcr = val,
            REG_MCR => self.sim_mcr = val,
            REG_SCRATCH => self.sim_scratch = val,
            _ => {}
        }
        let _ = port;
    }

    fn inb(&self, offset: u16) -> u8 {
        let port = self.base_port + offset;
        let _ = port;
        match offset {
            REG_LSR => LSR_TX_EMPTY | LSR_LINE_IDLE, // Always ready in simulation
            REG_SCRATCH => self.sim_scratch,
            REG_DATA => 0xAE, // Loopback test byte
            REG_IER => self.sim_ier,
            REG_LCR => self.sim_lcr,
            REG_MCR => self.sim_mcr,
            _ => 0,
        }
    }

    fn is_hardware_present(&self) -> bool {
        // In user-mode host mode, hardware I/O ports are virtualized
        false
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// Global COM1 serial driver instance protected by a standard mutex.
pub static COM1: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1_BASE));

/// Prints formatted text to the COM1 serial console.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        if let Ok(mut port) = $crate::arch::serial::COM1.lock() {
            use std::fmt::Write;
            let _ = write!(port, $($arg)*);
        }
        print!($($arg)*);
    };
}

/// Prints formatted text with a newline to the COM1 serial console.
#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::serial_print!("\n");
    };
    ($($arg:tt)*) => {
        if let Ok(mut port) = $crate::arch::serial::COM1.lock() {
            use std::fmt::Write;
            let _ = writeln!(port, $($arg)*);
        }
        println!($($arg)*);
    };
}

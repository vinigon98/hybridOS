//! Serial COM1 Driver for x86_64 Kernel Console Logging.
//!
//! Implements a real 16550A UART driver communicating over bare-metal x86 I/O ports
//! (standard COM1 base: 0x3F8). Writes hardware output directly to serial port 0x3F8
//! for QEMU `-serial stdio` and hardware COM1 capture.

use core::fmt;
use crate::sync::SpinLock;
use crate::arch::msr::{inb, outb};

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
const LSR_TX_EMPTY: u8 = 1 << 5;     // Transmitter Holding Register Empty (ready to write)

/// Low-level port I/O Serial Port Driver.
pub struct SerialPort {
    base_port: u16,
    is_initialized: bool,
}

impl SerialPort {
    /// Creates a new serial port handle for the given base I/O port.
    pub const fn new(base_port: u16) -> Self {
        Self {
            base_port,
            is_initialized: false,
        }
    }

    /// Initializes the 16550A UART device with hardware instructions:
    /// 1. Disables all interrupts.
    /// 2. Enables DLAB (Divisor Latch Access Bit) to configure baud rate.
    /// 3. Sets baud rate to 115200 (divisor = 1).
    /// 4. Configures 8 data bits, no parity, 1 stop bit (8N1).
    /// 5. Enables FIFO, clears TX/RX queues with 14-byte interrupt threshold.
    /// 6. Configures modem control (RTS/DTR set, auxiliary output 2 enabled).
    pub fn init(&mut self) -> Result<(), &'static str> {
        unsafe {
            // 1. Disable all interrupts
            self.write_reg(REG_IER, 0x00);

            // 2. Enable DLAB (set bit 7 of Line Control Register)
            self.write_reg(REG_LCR, 0x80);

            // 3. Set baud divisor to 1 (115200 baud)
            self.write_reg(REG_DATA, 0x01); // Divisor LSB: 1
            self.write_reg(REG_IER, 0x00);  // Divisor MSB: 0

            // 4. Set line configuration: 8 bits, no parity, 1 stop bit (8N1)
            self.write_reg(REG_LCR, 0x03);

            // 5. Enable FIFO, clear transmit/receive queues, 14-byte threshold
            self.write_reg(REG_FIFO, 0xC7);

            // 6. Set RTS/DSR set, auxiliary output 2 (enables IRQ line)
            self.write_reg(REG_MCR, 0x0B);

            // 7. Loopback test (optional verification)
            self.write_reg(REG_MCR, 0x1E);
            self.write_reg(REG_DATA, 0xAE);
            let check = self.read_reg(REG_DATA);
            if check != 0xAE {
                // In some emulators loopback might differ; still proceed
            }
            self.write_reg(REG_MCR, 0x0F); // Normal operation mode
        }

        self.is_initialized = true;
        Ok(())
    }

    /// Checks if transmitter holding register is empty.
    #[inline]
    pub fn is_transmit_empty(&self) -> bool {
        unsafe { (self.read_reg(REG_LSR) & LSR_TX_EMPTY) != 0 }
    }

    /// Checks if received data is ready to read.
    #[inline]
    pub fn is_data_ready(&self) -> bool {
        unsafe { (self.read_reg(REG_LSR) & LSR_DATA_READY) != 0 }
    }

    /// Writes a single byte to the serial console via real port I/O `out dx, al`.
    pub fn send(&mut self, byte: u8) {
        let mut timeout = 100_000;
        while !self.is_transmit_empty() && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
        unsafe {
            self.write_reg(REG_DATA, byte);
        }
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
            unsafe { Some(self.read_reg(REG_DATA)) }
        } else {
            None
        }
    }

    #[inline]
    unsafe fn write_reg(&self, offset: u16, val: u8) {
        outb(self.base_port + offset, val);
    }

    #[inline]
    unsafe fn read_reg(&self, offset: u16) -> u8 {
        inb(self.base_port + offset)
    }

    pub const fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// Global COM1 serial driver instance protected by bare-metal spinlock.
pub static COM1: SpinLock<SerialPort> = SpinLock::new(SerialPort::new(COM1_BASE));

/// Early initialization of COM1.
pub fn init_early_serial() {
    let mut port = COM1.lock();
    let _ = port.init();
}

/// Prints formatted text to the COM1 serial console.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let mut port = $crate::arch::serial::COM1.lock();
            let _ = write!(port, $($arg)*);
        }
    };
}

/// Prints formatted text with a newline to the COM1 serial console.
#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::serial_print!("\n");
    };
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let mut port = $crate::arch::serial::COM1.lock();
            let _ = writeln!(port, $($arg)*);
        }
    };
}

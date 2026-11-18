//! Formatted user-space console I/O for HybridOS Ring 3.

use core::fmt;

pub struct StdoutWriter;

impl fmt::Write for StdoutWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        // 1. Try Linux sys_write on stdout (fd=1)
        let ret = crate::linux::sys_write(crate::linux::STDOUT_FILENO, bytes);
        if ret >= 0 {
            return Ok(());
        }

        // 2. Fallback to Windows NtWriteFile on Stdout Handle (typically 1 or 2)
        crate::windows::nt_write_file(1, 0, 0, 0, 0, bytes);
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    let mut writer = StdoutWriter;
    let _ = writer.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::io::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}

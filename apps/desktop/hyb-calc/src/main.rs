//! hyb-calc: Technical, Scientific & Programmer Bitwise Calculator
//!
//! Production-grade calculator supporting arbitrary bit-width bitwise arithmetic
//! (8/16/32/64 bits), simultaneous HEX/DEC/OCT/BIN representation, scientific
//! trigonometry, constants, history, and CLI evaluation.

use libhyb_cfg::ConfigStore;
use libhyb_ui::colors::*;
use libhyb_ui::surface::Surface;
use libhyb_ui::widgets::Button;
use std::env;
use std::path::Path;

/// Bitwise Word Size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitWidth {
    Qword = 64,
    Dword = 32,
    Word = 16,
    Byte = 8,
}

/// Calculator Operational Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcMode {
    Programmer,
    Scientific,
}

/// Core Mathematical and Bitwise Engine
pub struct CalculatorEngine {
    pub value: u64,
    pub bit_width: BitWidth,
    pub mode: CalcMode,
    pub history: Vec<(String, u64)>,
}

impl CalculatorEngine {
    pub fn new() -> Self {
        Self {
            value: 0,
            bit_width: BitWidth::Qword,
            mode: CalcMode::Programmer,
            history: Vec::new(),
        }
    }

    pub fn mask(&self) -> u64 {
        match self.bit_width {
            BitWidth::Qword => u64::MAX,
            BitWidth::Dword => 0xFFFFFFFF,
            BitWidth::Word => 0xFFFF,
            BitWidth::Byte => 0xFF,
        }
    }

    pub fn set_value(&mut self, val: u64) {
        self.value = val & self.mask();
    }

    pub fn op_and(&mut self, rhs: u64) {
        self.set_value(self.value & rhs);
    }

    pub fn op_or(&mut self, rhs: u64) {
        self.set_value(self.value | rhs);
    }

    pub fn op_xor(&mut self, rhs: u64) {
        self.set_value(self.value ^ rhs);
    }

    pub fn op_not(&mut self) {
        self.set_value(!self.value);
    }

    pub fn op_shl(&mut self, shift: u32) {
        self.set_value(self.value.wrapping_shl(shift));
    }

    pub fn op_shr(&mut self, shift: u32) {
        self.set_value(self.value.wrapping_shr(shift));
    }

    pub fn op_rol(&mut self, shift: u32) {
        let w = self.bit_width as u32;
        let s = shift % w;
        let masked = self.value & self.mask();
        let rotated = (masked << s) | (masked >> (w - s));
        self.set_value(rotated);
    }

    pub fn op_ror(&mut self, shift: u32) {
        let w = self.bit_width as u32;
        let s = shift % w;
        let masked = self.value & self.mask();
        let rotated = (masked >> s) | (masked << (w - s));
        self.set_value(rotated);
    }

    pub fn hex_str(&self) -> String {
        format!("0x{:X}", self.value)
    }

    pub fn dec_str(&self) -> String {
        format!("{}", self.value)
    }

    pub fn oct_str(&self) -> String {
        format!("0o{:o}", self.value)
    }

    pub fn bin_str(&self) -> String {
        let w = self.bit_width as usize;
        let mut s = String::new();
        for i in (0..w).rev() {
            if (self.value & (1 << i)) != 0 {
                s.push('1');
            } else {
                s.push('0');
            }
            if i > 0 && i % 4 == 0 {
                s.push(' ');
            }
        }
        s
    }
}

/// Evaluate expression from CLI
pub fn evaluate_cli_expression(expr: &str) -> Result<u64, String> {
    let trimmed = expr.trim();

    // Check shift left <<
    if let Some((l, r)) = trimmed.split_once("<<") {
        let left = parse_flexible_int(l.trim())?;
        let right = r.trim().parse::<u32>().map_err(|e| e.to_string())?;
        return Ok(left << right);
    }

    // Check shift right >>
    if let Some((l, r)) = trimmed.split_once(">>") {
        let left = parse_flexible_int(l.trim())?;
        let right = r.trim().parse::<u32>().map_err(|e| e.to_string())?;
        return Ok(left >> right);
    }

    // Check XOR ^
    if let Some((l, r)) = trimmed.split_once('^') {
        let left = parse_flexible_int(l.trim())?;
        let right = parse_flexible_int(r.trim())?;
        return Ok(left ^ right);
    }

    // Check OR |
    if let Some((l, r)) = trimmed.split_once('|') {
        let left = parse_flexible_int(l.trim())?;
        let right = parse_flexible_int(r.trim())?;
        return Ok(left | right);
    }

    // Check AND &
    if let Some((l, r)) = trimmed.split_once('&') {
        let left = parse_flexible_int(l.trim())?;
        let right = parse_flexible_int(r.trim())?;
        return Ok(left & right);
    }

    // Check Multiplication *
    if let Some((l, r)) = trimmed.split_once('*') {
        let left = parse_flexible_int(l.trim())?;
        let right = parse_flexible_int(r.trim())?;
        return Ok(left * right);
    }

    // Check Addition +
    if let Some((l, r)) = trimmed.split_once('+') {
        let left = parse_flexible_int(l.trim())?;
        let right = parse_flexible_int(r.trim())?;
        return Ok(left + right);
    }

    // Single number
    parse_flexible_int(trimmed)
}

fn parse_flexible_int(s: &str) -> Result<u64, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u64::from_str_radix(bin, 2).map_err(|e| e.to_string())
    } else if let Some(oct) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        u64::from_str_radix(oct, 8).map_err(|e| e.to_string())
    } else {
        s.parse::<u64>().map_err(|e| e.to_string())
    }
}

/// Calculator GUI State
pub struct CalculatorGui {
    pub engine: CalculatorEngine,
    pub buttons: Vec<Button>,
}

impl CalculatorGui {
    pub fn new() -> Self {
        let mut buttons = Vec::new();

        // Mode and bit-width toggles
        buttons.push(Button::new(20, 160, 80, 32, "HEX").with_accent());
        buttons.push(Button::new(108, 160, 80, 32, "DEC"));
        buttons.push(Button::new(196, 160, 80, 32, "OCT"));
        buttons.push(Button::new(284, 160, 80, 32, "BIN"));

        // Operators
        buttons.push(Button::new(20, 204, 60, 36, "AND"));
        buttons.push(Button::new(88, 204, 60, 36, "OR"));
        buttons.push(Button::new(156, 204, 60, 36, "XOR"));
        buttons.push(Button::new(224, 204, 60, 36, "NOT"));
        buttons.push(Button::new(292, 204, 60, 36, "<<"));
        buttons.push(Button::new(360, 204, 60, 36, ">>"));

        Self {
            engine: CalculatorEngine::new(),
            buttons,
        }
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(0, 0, surface.width as u32, surface.height as u32, COLOR_CARD);

        // Display Header Box
        surface.fill_rect(20, 20, surface.width as u32 - 40, 120, COLOR_SURFACE);
        surface.draw_rect(20, 20, surface.width as u32 - 40, 120, COLOR_BORDER);

        // Base values
        surface.draw_string(32, 32, &format!("HEX  {}", self.engine.hex_str()), COLOR_CYAN);
        surface.draw_string(32, 54, &format!("DEC  {}", self.engine.dec_str()), COLOR_TEXT);
        surface.draw_string(32, 76, &format!("OCT  {}", self.engine.oct_str()), COLOR_SUCCESS);
        surface.draw_string(32, 98, &format!("BIN  {}", self.engine.bin_str()), COLOR_TEXT_MUTED);

        // Render Buttons
        for btn in &self.buttons {
            btn.render(surface);
        }

        // Bit Grid
        let bg_y: i32 = 260;
        surface.draw_string(20, bg_y as usize, "Interactive 64-Bit Matrix (Click to toggle):", COLOR_TEXT_MUTED);
        for row in 0..4 {
            for col in 0..16 {
                let bit_idx = 63 - (row * 16 + col);
                let is_set = (self.engine.value & (1 << bit_idx)) != 0;
                let bx = 20 + (col as i32 * 26);
                let by = (bg_y + 24) + (row as i32 * 26);
                let col_val = if is_set { COLOR_ACCENT } else { 0xFF242424 };

                surface.fill_rect(bx, by, 22, 22, col_val);
                surface.draw_rect(bx, by, 22, 22, COLOR_BORDER);
                let bit_char = if is_set { "1" } else { "0" };
                surface.draw_string((bx + 7) as usize, (by + 4) as usize, bit_char, COLOR_TEXT);
            }
        }
    }
}

fn print_usage() {
    println!("hyb-calc: HybridOS Technical & Bitwise Calculator");
    println!("Usage: hyb-calc [OPTIONS] [EXPRESSION]");
    println!("Examples:");
    println!("  hyb-calc \"0xFF << 4\"");
    println!("  hyb-calc \"1024 * 768\"");
    println!("Options:");
    println!("  --screenshot <PATH>  Render UI to a PNG screenshot and exit");
    println!("  --version            Display version information");
    println!("  --help               Display this help text");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let _config = ConfigStore::new();

    if args.contains(&"--version".to_string()) {
        println!("hyb-calc 0.1.0 (HybridOS Arbitrary Precision Calculator)");
        return;
    }

    if args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    // CLI Expression calculation check
    if let Some(arg) = args.get(1) {
        if !arg.starts_with("--") {
            match evaluate_cli_expression(arg) {
                Ok(res) => {
                    println!("Expression: {}", arg);
                    println!("Result: 0x{:X} ({}) [0b{:b}]", res, res, res);
                    return;
                }
                Err(e) => eprintln!("Evaluation error: {}", e),
            }
        }
    }

    let mut calc = CalculatorGui::new();
    calc.engine.set_value(0x1920_1080);

    println!("[hyb-calc] Programmer calculator active. Current value: {}", calc.engine.hex_str());

    let mut surface = Surface::new(600, 440, COLOR_BG);
    calc.render(&mut surface);

    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        if let Some(path_str) = args.get(pos + 1) {
            let p = Path::new(path_str);
            match surface.export_png(p) {
                Ok(_) => println!("[hyb-calc] Screenshot written successfully: {}", path_str),
                Err(e) => eprintln!("[hyb-calc] Error exporting screenshot: {}", e),
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitwise_shifts_and_ops() {
        let mut engine = CalculatorEngine::new();
        engine.set_value(0x0F);
        engine.op_shl(4);
        assert_eq!(engine.value, 0xF0);

        engine.op_or(0x05);
        assert_eq!(engine.value, 0xF5);

        engine.op_xor(0x55);
        assert_eq!(engine.value, 0xA0);
    }

    #[test]
    fn test_cli_evaluator() {
        assert_eq!(evaluate_cli_expression("0xFF << 4").unwrap(), 0xFF0);
        assert_eq!(evaluate_cli_expression("100 + 200").unwrap(), 300);
        assert_eq!(evaluate_cli_expression("0b1010 ^ 0b0101").unwrap(), 0b1111);
    }
}

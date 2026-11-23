//! libhyb-ui: High-Performance GPU/DRM & Software Compositor UI Framework

pub mod font;
pub mod geometry;
pub mod icons;
pub mod surface;
pub mod widgets;

pub use font::*;
pub use geometry::*;
pub use icons::*;
pub use surface::*;
pub use widgets::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_intersections() {
        let r1 = Rect::new(10, 10, 100, 100);
        let r2 = Rect::new(50, 50, 100, 100);
        let r3 = Rect::new(200, 200, 50, 50);

        assert!(r1.intersects(&r2));
        assert!(r2.intersects(&r1));
        assert!(!r1.intersects(&r3));
    }

    #[test]
    fn test_surface_rendering_and_png_export() {
        let mut surface = Surface::new(64, 64, colors::COLOR_BG);
        surface.fill_rect(10, 10, 44, 44, colors::COLOR_ACCENT);
        surface.draw_rect(10, 10, 44, 44, colors::COLOR_TEXT);
        surface.draw_string(14, 24, "HYB", colors::COLOR_TEXT);

        let temp_png = std::env::temp_dir().join("test_ui_surface.png");
        let res = surface.export_png(&temp_png);
        assert!(res.is_ok());
        assert!(temp_png.exists());
        let _ = std::fs::remove_file(temp_png);
    }

    #[test]
    fn test_button_interaction() {
        let mut btn = Button::new(20, 20, 100, 30, "Confirm");
        assert!(!btn.is_hovered);
        assert!(!btn.is_pressed);

        let clicked = btn.handle_mouse(Point::new(30, 25), true);
        assert!(btn.is_hovered);
        assert!(btn.is_pressed);
        assert!(!clicked); // Still holding mouse button

        let released_click = btn.handle_mouse(Point::new(30, 25), false);
        assert!(btn.is_hovered);
        assert!(!btn.is_pressed);
        assert!(released_click); // Click completed on release!
    }

    #[test]
    fn test_table_sorting() {
        let mut table = TableView::new(
            0,
            0,
            300,
            200,
            vec![("Name", 150), ("Size", 100)],
        );
        table.add_row(vec!["kernel.bin", "1024"]);
        table.add_row(vec!["app.elf", "512"]);
        table.add_row(vec!["boot.ini", "128"]);

        // First click flips to descending sort
        table.handle_click(Point::new(10, 10));
        assert_eq!(table.rows[0][0], "kernel.bin");
        assert_eq!(table.rows[1][0], "boot.ini");
        assert_eq!(table.rows[2][0], "app.elf");

        // Second click flips back to ascending sort
        table.handle_click(Point::new(10, 10));
        assert_eq!(table.rows[0][0], "app.elf");
        assert_eq!(table.rows[1][0], "boot.ini");
        assert_eq!(table.rows[2][0], "kernel.bin");
    }
}

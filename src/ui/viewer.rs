//! Image viewer: the overlay a click on a picture or a diagram opens.
//! The media sits over a scrim at the largest size that fits, natural
//! size when it is smaller than the window; the wheel zooms, a drag
//! pans, and the caption names the file and its pixels.

use image::RgbaImage;
use winit::keyboard::{Key, NamedKey};

use crate::paint::painter::Painter;
use crate::style::fonts::CODE_FAMILY;
use crate::style::theme::Theme;
use crate::ui::overlay::{self, Overlay, OverlayResult};
use crate::ui::search;

/// Room around the picture, in logical units.
const MARGIN: f32 = 32.0;
/// The band the caption sits in at the bottom.
const CAPTION_H: f32 = 44.0;
/// One wheel notch or zoom key step.
const ZOOM_STEP: f32 = 1.25;
/// The most a zoom reaches, eight times natural size.
const ZOOM_MAX: f32 = 8.0;
/// One arrow key pan, in logical units.
const PAN: f32 = 60.0;

/// The viewer over the page: the media's own pixels, where it sits on
/// the screen, and the pan while it is zoomed past the window.
pub struct Viewer {
    /// The media's pixels at natural resolution, straight RGBA.
    pixels: RgbaImage,
    /// The caption: the file's name, or `diagram` for a rendered one.
    name: String,
    /// Zoom relative to natural size; 1.0 is natural size.
    zoom: f32,
    /// The pan from the centered seat, in logical units.
    offset: (f32, f32),
    /// The grab point while the button is held on the image.
    grab: Option<(f32, f32)>,
    /// The resized copy at the last drawn physical size.
    scaled: Option<(u32, u32, Vec<u8>)>,
    /// The image's place at the last draw, for hit testing.
    placed: (f32, f32, f32, f32),
    /// The window size at the last draw, in logical units.
    window: (f32, f32),
}

impl Viewer {
    pub fn new(pixels: Vec<u8>, width: u32, height: u32, name: String) -> Viewer {
        Viewer {
            pixels: RgbaImage::from_raw(width, height, pixels).unwrap_or_default(),
            name,
            zoom: 1.0,
            offset: (0.0, 0.0),
            grab: None,
            scaled: None,
            placed: (0.0, 0.0, 0.0, 0.0),
            window: (0.0, 0.0),
        }
    }

    /// The zoom that fits the media in the window, never above natural
    /// size: a small picture shows as it is rather than blown up soft.
    fn fit(&self, window: (f32, f32)) -> f32 {
        let avail = (window.0 - 2.0 * MARGIN, window.1 - 2.0 * MARGIN - CAPTION_H);
        let by_width = avail.0.max(1.0) / self.pixels.width() as f32;
        let by_height = avail.1.max(1.0) / self.pixels.height() as f32;
        by_width.min(by_height).min(1.0)
    }

    /// The image's rect, origin, width and height in logical units:
    /// the centered seat moved by the pan.
    fn rect(&self, window: (f32, f32)) -> (f32, f32, f32, f32) {
        let w = self.pixels.width() as f32 * self.zoom;
        let h = self.pixels.height() as f32 * self.zoom;
        let x = (window.0 - w) / 2.0 + self.offset.0;
        let y = (window.1 - CAPTION_H - h) / 2.0 + self.offset.1;
        (x, y, w, h)
    }

    /// The pan the rect allows: the picture keeps covering an axis
    /// while it is larger than the window, and centers once it is not.
    fn clamp(&self, offset: (f32, f32), window: (f32, f32)) -> (f32, f32) {
        let (_, _, w, h) = self.rect(window);
        let reach = |size: f32, window: f32| (size - window).max(0.0) / 2.0;
        (
            offset.0.clamp(-reach(w, window.0), reach(w, window.0)),
            offset.1.clamp(-reach(h, window.1), reach(h, window.1)),
        )
    }

    /// Zooms by `factor` around the window's center: the point of the
    /// picture under the center stays under it. The pan follows the
    /// zoom so the picture grows from where it sits, then clamps.
    /// Before a first draw sets the window, the zoom holds: there is
    /// nothing to fit yet, and no input can precede the first frame.
    fn zoom_by(&mut self, factor: f32) {
        if self.window.0 < 1.0 {
            return;
        }
        let min = self.fit(self.window);
        let was = self.zoom;
        self.zoom = (self.zoom * factor).clamp(min, ZOOM_MAX);
        if self.zoom != was {
            self.offset = (
                self.offset.0 * self.zoom / was,
                self.offset.1 * self.zoom / was,
            );
            self.offset = self.clamp(self.offset, self.window);
        }
    }

    /// Back to the fit size, or to natural size with `natural`, the
    /// pan cleared either way.
    fn reset(&mut self, natural: bool) {
        self.zoom = if natural { 1.0 } else { self.fit(self.window) };
        self.offset = (0.0, 0.0);
    }
}

impl Overlay for Viewer {
    fn draw(&mut self, painter: &mut Painter, theme: &Theme) {
        let window = (painter.width(), painter.height());
        self.window = window;
        if self.scaled.is_none() {
            // The first draw opens at fit, not at natural size.
            self.zoom = self.fit(window);
        }
        painter.fill(0.0, 0.0, window.0, window.1, 0.0, overlay::SCRIM);
        let (x, y, w, h) = self.rect(window);
        self.placed = (x, y, w, h);

        // The copy the canvas blits: resized only when its size moved,
        // at the physical size the rect covers, so the blit is exact.
        let pw = ((w * painter.scale()).round() as u32).max(1);
        let ph = ((h * painter.scale()).round() as u32).max(1);
        if self
            .scaled
            .as_ref()
            .is_none_or(|(sw, sh, _)| *sw != pw || *sh != ph)
        {
            let resized = image::imageops::resize(
                &self.pixels,
                pw,
                ph,
                image::imageops::FilterType::Lanczos3,
            );
            self.scaled = Some((pw, ph, resized.into_raw()));
        }
        if let Some((sw, sh, rgba)) = self.scaled.as_ref() {
            painter.image(rgba, x, y, w, h, *sw, *sh);
        }

        // The caption names the file, its pixels and the zoom.
        let caption = format!(
            "{}  ·  {}×{}  ·  {}%",
            self.name,
            self.pixels.width(),
            self.pixels.height(),
            (self.zoom * 100.0).round() as u32
        );
        let text_w = painter.measure(&caption, CODE_FAMILY, search::COUNTER_SIZE, 400);
        let pill_w = text_w + 2.0 * search::PAD;
        let pill_x = ((window.0 - pill_w) / 2.0).max(MARGIN);
        let pill_y = window.1 - CAPTION_H + (CAPTION_H - search::BAR_HEIGHT) / 2.0;
        painter.fill(
            pill_x,
            pill_y,
            pill_w,
            search::BAR_HEIGHT,
            search::RADIUS,
            theme.ui.overlay_bg,
        );
        painter.text(
            pill_x + search::PAD,
            pill_y + 10.0 + search::TEXT_DROP,
            &caption,
            CODE_FAMILY,
            search::COUNTER_SIZE,
            400,
            theme.ui.overlay_fg,
        );
    }

    fn key(&mut self, key: &Key, _ctrl: bool, _shift: bool) -> OverlayResult {
        match key {
            Key::Named(NamedKey::Escape) => OverlayResult::Close,
            Key::Character(text) if matches!(text.as_str(), "+" | "=") => {
                self.zoom_by(ZOOM_STEP);
                OverlayResult::Open
            }
            Key::Character(text) if text.as_str() == "-" => {
                self.zoom_by(1.0 / ZOOM_STEP);
                OverlayResult::Open
            }
            Key::Character(text) if text.as_str() == "0" => {
                self.reset(false);
                OverlayResult::Open
            }
            Key::Character(text) if text.as_str() == "1" => {
                self.reset(true);
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.offset.0 += PAN;
                self.offset = self.clamp(self.offset, self.window);
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.offset.0 -= PAN;
                self.offset = self.clamp(self.offset, self.window);
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.offset.1 += PAN;
                self.offset = self.clamp(self.offset, self.window);
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.offset.1 -= PAN;
                self.offset = self.clamp(self.offset, self.window);
                OverlayResult::Open
            }
            _ => OverlayResult::Open,
        }
    }

    fn click(&mut self, x: f32, y: f32) -> OverlayResult {
        let (ix, iy, iw, ih) = self.placed;
        let inside = x >= ix && x <= ix + iw && y >= iy && y <= iy + ih;
        if inside {
            self.grab = Some((x, y));
            OverlayResult::Open
        } else {
            // A click away from the picture closes, the dialogs' own
            // outside-click manner.
            OverlayResult::Close
        }
    }

    fn drag(&mut self, x: f32, y: f32) -> OverlayResult {
        if let Some((from_x, from_y)) = self.grab {
            self.offset.0 += x - from_x;
            self.offset.1 += y - from_y;
            self.grab = Some((x, y));
            self.offset = self.clamp(self.offset, self.window);
        }
        OverlayResult::Open
    }

    fn release(&mut self) {
        self.grab = None;
    }

    fn scroll(&mut self, lines: f32) -> OverlayResult {
        let steps = ZOOM_STEP.powf(lines.abs());
        self.zoom_by(if lines > 0.0 { 1.0 / steps } else { steps });
        OverlayResult::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(w: u32, h: u32) -> Viewer {
        Viewer::new(vec![0; w as usize * h as usize * 4], w, h, "pic.png".into())
    }

    /// The viewer a first frame leaves behind, armed for the input
    /// handlers the way a draw does.
    fn drawn(mut view: Viewer, window: (f32, f32)) -> Viewer {
        view.window = window;
        view.zoom = view.fit(window);
        view.placed = view.rect(window);
        view
    }

    #[test]
    fn a_big_picture_fits_the_window() {
        let view = viewer(4000, 3000);
        // The taller side rules: (800 - 64 - 44) / 3000.
        assert!((view.fit((1200.0, 800.0)) - 0.2307).abs() < 0.001);
    }

    #[test]
    fn a_small_picture_stays_at_natural_size() {
        let view = viewer(200, 100);
        assert_eq!(view.fit((1200.0, 800.0)), 1.0, "no upscale at fit");
    }

    #[test]
    fn the_rect_is_centered_until_pan() {
        let mut view = viewer(400, 200);
        let (x, y, w, h) = view.rect((1000.0, 600.0));
        assert_eq!((w, h), (400.0, 200.0));
        assert_eq!(x, 300.0);
        assert_eq!(y, 178.0, "the caption band lifts the seat");
        view.offset = (40.0, -20.0);
        let (x, y, _, _) = view.rect((1000.0, 600.0));
        assert_eq!((x, y), (340.0, 158.0));
    }

    #[test]
    fn a_pan_cannot_uncover_the_scrim() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 0.5;
        let clamped = view.clamp((5000.0, -5000.0), (1000.0, 600.0));
        assert_eq!(clamped, (500.0, -450.0));
        // Smaller than the window: every axis centers.
        view.zoom = 0.1;
        assert_eq!(view.clamp((300.0, 300.0), (1000.0, 600.0)), (0.0, 0.0));
    }

    #[test]
    fn zoom_stops_at_fit_and_eight_times() {
        let mut view = drawn(viewer(4000, 3000), (1200.0, 800.0));
        for _ in 0..4 {
            view.zoom_by(1.0 / ZOOM_STEP);
        }
        assert!(
            (view.zoom - view.fit((1200.0, 800.0))).abs() < 1e-6,
            "no smaller than fit"
        );
        for _ in 0..40 {
            view.zoom_by(ZOOM_STEP);
        }
        assert!((view.zoom - ZOOM_MAX).abs() < 1e-6);
    }

    #[test]
    fn zoom_keeps_the_picture_point_under_the_center() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        view.offset = (100.0, 60.0);
        view.zoom_by(2.0);
        assert!((view.zoom - 2.0).abs() < 1e-6);
        // The pan rides the zoom: the same natural point stays under
        // the window's center, offset over zoom unchanged.
        assert!((view.offset.0 / view.zoom - 100.0).abs() < 1e-3);
        assert!((view.offset.1 / view.zoom - 60.0).abs() < 1e-3);
    }

    #[test]
    fn zoom_before_the_first_draw_holds() {
        let mut view = viewer(4000, 3000);
        view.zoom_by(ZOOM_STEP);
        assert_eq!(view.zoom, 1.0, "no window yet, nothing to fit");
    }

    #[test]
    fn the_wheel_and_the_keys_agree_on_the_step() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        view.scroll(-3.0);
        let wheel = view.zoom;
        view.zoom = 1.0;
        for _ in 0..3 {
            view.zoom_by(ZOOM_STEP);
        }
        assert!((wheel - view.zoom).abs() < 1e-6);
    }

    #[test]
    fn a_click_on_the_picture_grabs_and_away_closes() {
        let mut view = viewer(100, 100);
        view.placed = (100.0, 100.0, 400.0, 300.0);
        assert!(matches!(view.click(200.0, 200.0), OverlayResult::Open));
        assert_eq!(view.grab, Some((200.0, 200.0)));
        assert!(matches!(view.click(50.0, 50.0), OverlayResult::Close));
        view.release();
        assert_eq!(view.grab, None);
    }

    #[test]
    fn a_drag_pans_by_the_pointer_and_clamps() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        let seat = view.rect((1000.0, 600.0));
        view.placed = seat;
        view.click(500.0, 300.0);
        view.drag(560.0, 280.0);
        let (x, y, _, _) = view.rect((1000.0, 600.0));
        assert_eq!(x, seat.0 + 60.0);
        assert_eq!(y, seat.1 - 20.0);
    }

    #[test]
    fn the_arrows_pan_and_clamp() {
        use winit::keyboard::NamedKey;
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        view.placed = view.rect((1000.0, 600.0));
        view.key(&Key::Named(NamedKey::ArrowLeft), false, false);
        assert_eq!(view.offset.0, PAN, "a big picture pans");
        view.key(&Key::Named(NamedKey::ArrowRight), false, false);
        assert_eq!(view.offset.0, 0.0, "and the other way back");
        view.zoom = 0.1;
        view.placed = view.rect((1000.0, 600.0));
        view.key(&Key::Named(NamedKey::ArrowLeft), false, false);
        assert_eq!(view.offset.0, 0.0, "a small picture stays centered");
    }

    #[test]
    fn the_zoom_keys_and_escape_read_plain() {
        use winit::keyboard::NamedKey;
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        view.key(&Key::Character("+".into()), false, false);
        assert!((view.zoom - ZOOM_STEP).abs() < 1e-6);
        view.key(&Key::Character("=".into()), false, false);
        assert!((view.zoom - ZOOM_STEP * ZOOM_STEP).abs() < 1e-6);
        view.key(&Key::Character("-".into()), false, false);
        assert!((view.zoom - ZOOM_STEP).abs() < 1e-6);
        view.key(&Key::Character("1".into()), false, false);
        assert_eq!(view.zoom, 1.0, "1 is natural size");
        view.key(&Key::Character("0".into()), false, false);
        assert_eq!(view.zoom, view.fit((1000.0, 600.0)), "0 fits");
        assert!(matches!(
            view.key(&Key::Named(NamedKey::Escape), false, false),
            OverlayResult::Close
        ));
    }
}

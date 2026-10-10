//! Image viewer: the overlay a click on a picture or a diagram opens.
//! The media sits over a scrim at the largest size that fits, natural
//! size when it is smaller than the window; the wheel zooms under the
//! pointer, a drag pans, and the caption names the file and its pixels.

use std::borrow::Cow;
use std::time::{Duration, Instant};

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
/// How long after the last zoom or pan the sharp frame draws: the
/// wheel turns faster than a resample can follow, so frames in motion
/// blit the picture's own pixels and the sharp one lands once they
/// stop.
const REFINE_DELAY: Duration = Duration::from_millis(160);
/// How many times the window's own pixels a whole-picture resample may
/// cover. Deeper than that, the resample trims to what shows, so both
/// its cost and its memory stay bounded by the window.
const FULL_MARGIN: u64 = 2;

/// A resampled buffer and the part of the picture it holds: the whole
/// picture, or the natural-rect `spot` names for the trimmed kind.
struct Sharp {
    w: u32,
    h: u32,
    bytes: Vec<u8>,
    /// The natural-rect the bytes cover; the whole picture for `None`.
    spot: Option<(u32, u32, u32, u32)>,
}

/// What a frame at the current zoom owes. The picture's own pixels are
/// the sharp frame wherever they cover the screen one to one or
/// better; a shrinking view resamples, but only once motion stops.
enum Frame {
    /// Past natural size: blit the picture itself.
    Upscale,
    /// The cached whole-picture buffer already is this size.
    Exact,
    /// Motion: blit the picture, the cached trim over it when one
    /// still lines up.
    Rough,
    /// Resample the whole picture to the rect's physical size.
    Full,
    /// Resample the visible trim, the rect having grown past the cap.
    Crop,
}

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
    /// The sharp buffer at the last refined view.
    scaled: Option<Sharp>,
    /// The image's place at the last draw, for hit testing.
    placed: (f32, f32, f32, f32),
    /// The window size at the last draw, in logical units.
    window: (f32, f32),
    /// Whether a draw has set the window and the opening fit.
    sized: bool,
    /// When motion ends and the sharp frame may draw.
    motion_until: Instant,
    /// Whether the frame on screen is the rough draft.
    rough: bool,
    /// The system clipboard, woken on the first copy.
    clipboard: Option<arboard::Clipboard>,
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
            sized: false,
            // Opening is motion: the first frame blits the picture and
            // the sharp one follows the instant it opens.
            motion_until: Instant::now() + REFINE_DELAY,
            rough: false,
            clipboard: None,
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
            self.stir();
        }
    }

    /// Zooms by `factor` keeping the point of the picture under the
    /// anchor under it: the pointer is the anchor the wheel means. The
    /// clamp still wins at the edges, where the picture cannot follow.
    fn zoom_at(&mut self, factor: f32, anchor: (f32, f32)) {
        if self.window.0 < 1.0 {
            return;
        }
        let min = self.fit(self.window);
        let was = self.zoom;
        // The rect at the old zoom, read before the zoom moves: the
        // anchor's natural point measures from where the picture sits.
        let (x, y, _, _) = self.rect(self.window);
        self.zoom = (self.zoom * factor).clamp(min, ZOOM_MAX);
        if self.zoom != was {
            let nx = (anchor.0 - x) / was;
            let ny = (anchor.1 - y) / was;
            let w = self.pixels.width() as f32 * self.zoom;
            let h = self.pixels.height() as f32 * self.zoom;
            self.offset.0 = anchor.0 - nx * self.zoom - (self.window.0 - w) / 2.0;
            self.offset.1 = anchor.1 - ny * self.zoom - (self.window.1 - CAPTION_H - h) / 2.0;
            self.offset = self.clamp(self.offset, self.window);
            self.stir();
        }
    }

    /// Back to the fit size, or to natural size with `natural`, the
    /// pan cleared either way.
    fn reset(&mut self, natural: bool) {
        self.zoom = if natural { 1.0 } else { self.fit(self.window) };
        self.offset = (0.0, 0.0);
        self.stir();
    }

    /// Notes motion: the sharp frame waits for it to end.
    fn stir(&mut self) {
        self.motion_until = Instant::now() + REFINE_DELAY;
    }

    /// What the frame at `(pw, ph)`, the rect's physical size, owes;
    /// `ceil` is the whole-picture resample budget in pixels.
    fn frame(&self, pw: u32, ph: u32, ceil: u64, now: Instant) -> Frame {
        let nat = (self.pixels.width(), self.pixels.height());
        if pw >= nat.0 && ph >= nat.1 {
            return Frame::Upscale;
        }
        if matches!(&self.scaled, Some(s) if s.spot.is_none() && s.w == pw && s.h == ph) {
            return Frame::Exact;
        }
        if now < self.motion_until {
            return Frame::Rough;
        }
        if u64::from(pw) * u64::from(ph) <= ceil {
            Frame::Full
        } else {
            Frame::Crop
        }
    }

    /// The natural-rect the visible part shows, and the physical size
    /// it deserves: trimmed by the crop, proportions kept.
    fn trim(&self, scale: f32) -> ((u32, u32, u32, u32), (u32, u32), (f32, f32, f32, f32)) {
        let (x, y, w, h) = self.rect(self.window);
        let nat = (self.pixels.width(), self.pixels.height());
        // The picture's part the window keeps, in logical units.
        let left = x.max(0.0);
        let top = y.max(0.0);
        let right = (x + w).min(self.window.0);
        let bottom = (y + h).min(self.window.1 - CAPTION_H).max(top);
        // The same part in natural pixels, whole units, clamped in.
        let nx = (((left - x) / self.zoom).floor().max(0.0) as u32).min(nat.0 - 1);
        let ny = (((top - y) / self.zoom).floor().max(0.0) as u32).min(nat.1 - 1);
        let ex = (((right - x) / self.zoom).ceil().min(nat.0 as f32) as u32).max(nx + 1);
        let ey = (((bottom - y) / self.zoom).ceil().min(nat.1 as f32) as u32).max(ny + 1);
        let spot = (nx, ny, ex - nx, ey - ny);
        // Its size on screen: the trim's own pixels times the zoom,
        // rounded the way the blit rounds.
        let tw = (((ex - nx) as f32 * self.zoom * scale).round() as u32).max(1);
        let th = (((ey - ny) as f32 * self.zoom * scale).round() as u32).max(1);
        let sx = x + nx as f32 * self.zoom;
        let sy = y + ny as f32 * self.zoom;
        (
            spot,
            (tw, th),
            (
                sx,
                sy,
                (ex - nx) as f32 * self.zoom,
                (ey - ny) as f32 * self.zoom,
            ),
        )
    }

    /// Copies the picture to the system clipboard. The clipboard is
    /// commonly held by another party for a moment — a manager, a
    /// poller — so a refused attempt tries again before it gives up.
    fn copy(&mut self) {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(clipboard) = self.clipboard.as_mut() else {
            return;
        };
        for attempt in 0..3 {
            let data = arboard::ImageData {
                width: self.pixels.width() as usize,
                height: self.pixels.height() as usize,
                bytes: Cow::Borrowed(self.pixels.as_raw()),
            };
            match clipboard.set_image(data) {
                Ok(()) => return,
                Err(_) if attempt < 2 => std::thread::sleep(Duration::from_millis(40)),
                Err(err) => {
                    eprintln!("oryx: image copy failed: {err}");
                    return;
                }
            }
        }
    }
}

impl Overlay for Viewer {
    fn draw(&mut self, painter: &mut Painter, theme: &Theme) {
        let window = (painter.width(), painter.height());
        let scale = painter.scale();
        if !self.sized {
            // The first draw opens at fit, not at natural size.
            self.zoom = self.fit(window);
            self.sized = true;
        }
        let moved = window != self.window;
        self.window = window;
        if moved {
            // A resized window re-seats the pan before it draws.
            self.offset = self.clamp(self.offset, window);
            self.stir();
        }
        painter.fill(0.0, 0.0, window.0, window.1, 0.0, overlay::SCRIM);
        let (x, y, w, h) = self.rect(window);
        self.placed = (x, y, w, h);
        let nat = (self.pixels.width(), self.pixels.height());
        let pw = ((w * scale).round() as u32).max(1);
        let ph = ((h * scale).round() as u32).max(1);
        let ceil =
            FULL_MARGIN * ((window.0 * scale) as u64).max(1) * ((window.1 * scale) as u64).max(1);
        let plan = self.frame(pw, ph, ceil, Instant::now());
        match plan {
            Frame::Upscale => {
                // Every pixel of the picture covers one of the screen
                // or more: the picture itself is the sharp frame.
                self.scaled = None;
                self.rough = false;
                painter.image(self.pixels.as_raw(), x, y, w, h, nat.0, nat.1);
            }
            Frame::Exact => {
                self.rough = false;
                if let Some(s) = self.scaled.as_ref() {
                    painter.image(&s.bytes, x, y, w, h, s.w, s.h);
                }
            }
            Frame::Rough => {
                // The picture as it is, the cached trim over the part
                // it still lines up with, until motion ends.
                self.rough = true;
                painter.image(self.pixels.as_raw(), x, y, w, h, nat.0, nat.1);
                if let Some(s) = self.scaled.as_ref() {
                    if let Some(spot) = s.spot {
                        let (sx, sy, sw, sh) = (
                            x + spot.0 as f32 * self.zoom,
                            y + spot.1 as f32 * self.zoom,
                            spot.2 as f32 * self.zoom,
                            spot.3 as f32 * self.zoom,
                        );
                        painter.image(&s.bytes, sx, sy, sw, sh, s.w, s.h);
                    }
                }
            }
            Frame::Full => {
                let resized = image::imageops::resize(
                    &self.pixels,
                    pw,
                    ph,
                    image::imageops::FilterType::Lanczos3,
                );
                painter.image(&resized, x, y, w, h, pw, ph);
                self.scaled = Some(Sharp {
                    w: pw,
                    h: ph,
                    bytes: resized.into_raw(),
                    spot: None,
                });
                self.rough = false;
            }
            Frame::Crop => {
                let (spot, (tw, th), (sx, sy, sw, sh)) = self.trim(scale);
                let crop = image::imageops::crop_imm(&self.pixels, spot.0, spot.1, spot.2, spot.3);
                let resized = image::imageops::resize(
                    crop.inner(),
                    tw,
                    th,
                    image::imageops::FilterType::Lanczos3,
                );
                painter.image(&resized, sx, sy, sw, sh, tw, th);
                self.scaled = Some(Sharp {
                    w: tw,
                    h: th,
                    bytes: resized.into_raw(),
                    spot: Some(spot),
                });
                self.rough = false;
            }
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

    fn key(&mut self, key: &Key, ctrl: bool, _shift: bool) -> OverlayResult {
        match key {
            Key::Named(NamedKey::Escape) => OverlayResult::Close,
            Key::Character(text) if ctrl && text.eq_ignore_ascii_case("c") => {
                self.copy();
                OverlayResult::Open
            }
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
                self.stir();
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.offset.0 -= PAN;
                self.offset = self.clamp(self.offset, self.window);
                self.stir();
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.offset.1 += PAN;
                self.offset = self.clamp(self.offset, self.window);
                self.stir();
                OverlayResult::Open
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.offset.1 -= PAN;
                self.offset = self.clamp(self.offset, self.window);
                self.stir();
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
            self.stir();
        }
        OverlayResult::Open
    }

    fn release(&mut self) {
        self.grab = None;
    }

    fn scroll_at(&mut self, lines: f32, x: f32, y: f32) -> OverlayResult {
        let steps = ZOOM_STEP.powf(lines.abs());
        let factor = if lines > 0.0 { 1.0 / steps } else { steps };
        self.zoom_at(factor, (x, y));
        OverlayResult::Open
    }

    fn refine_at(&self) -> Option<Instant> {
        self.rough.then_some(self.motion_until)
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
        view.sized = true;
        view.zoom = view.fit(window);
        view.placed = view.rect(window);
        view
    }

    /// The viewer with motion over: the sharp frame is due.
    fn settled(mut view: Viewer) -> Viewer {
        view.motion_until = Instant::now() - REFINE_DELAY;
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
    fn the_wheel_zooms_where_the_pointer_is() {
        let mut view = settled(drawn(viewer(4000, 3000), (1000.0, 600.0)));
        view.zoom = 1.0;
        view.offset = (100.0, 60.0);
        let anchor = (700.0, 400.0);
        // The natural point under the anchor, before and after.
        let (x, y, _, _) = view.rect((1000.0, 600.0));
        let nx = (anchor.0 - x) / view.zoom;
        let ny = (anchor.1 - y) / view.zoom;
        view.zoom_at(2.0, anchor);
        assert!((view.zoom - 2.0).abs() < 1e-6);
        let (x, y, _, _) = view.rect((1000.0, 600.0));
        assert!((x + nx * view.zoom - anchor.0).abs() < 1e-3);
        assert!((y + ny * view.zoom - anchor.1).abs() < 1e-3);
    }

    #[test]
    fn an_anchored_zoom_yields_to_the_clamp_at_the_edges() {
        let mut view = settled(drawn(viewer(4000, 3000), (1000.0, 600.0)));
        view.zoom = 1.0;
        // A far-off anchor over the scrim: the picture still cannot
        // slide past the reach the clamp allows.
        view.zoom_at(2.0, (9000.0, -4000.0));
        assert!((view.zoom - 2.0).abs() < 1e-6);
        assert_eq!(view.offset, view.clamp(view.offset, (1000.0, 600.0)));
    }

    #[test]
    fn zoom_before_the_first_draw_holds() {
        let mut view = viewer(4000, 3000);
        view.zoom_by(ZOOM_STEP);
        assert_eq!(view.zoom, 1.0, "no window yet, nothing to fit");
        view.zoom_at(ZOOM_STEP, (10.0, 10.0));
        assert_eq!(view.zoom, 1.0);
    }

    #[test]
    fn the_wheel_and_the_keys_agree_on_the_step() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = 1.0;
        view.scroll_at(-3.0, 500.0, 300.0);
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

    #[test]
    fn control_c_copies_without_touching_the_zoom() {
        let mut view = drawn(viewer(40, 30), (1000.0, 600.0));
        view.zoom = 1.0;
        view.key(&Key::Character("c".into()), true, false);
        view.key(&Key::Character("C".into()), true, false);
        assert_eq!(view.zoom, 1.0, "the copy leaves the view alone");
    }

    #[test]
    fn the_clipboard_copy_carries_the_pixels() {
        // The data the copy sends is the picture's own RGBA, the same
        // buffer the frames blit.
        let view = Viewer::new(vec![7; 80 * 30 * 4], 80, 30, "dot.png".into());
        assert_eq!(view.pixels.width(), 80);
        assert_eq!(view.pixels.height(), 30);
        assert_eq!(view.pixels.as_raw().len(), 80 * 30 * 4);
        assert!(view.pixels.as_raw().iter().all(|&b| b == 7));
    }

    #[test]
    fn a_big_picture_blits_rough_while_moving_and_sharpens_after() {
        let mut view = drawn(viewer(4000, 3000), (1000.0, 600.0));
        view.zoom = view.fit((1000.0, 600.0));
        let pw = (4000.0 * view.zoom) as u32;
        let ph = (3000.0 * view.zoom) as u32;
        let ceil = FULL_MARGIN * 1000 * 600;
        // Motion holds the sharp frame back.
        view.stir();
        assert!(matches!(
            view.frame(pw, ph, ceil, Instant::now()),
            Frame::Rough
        ));
        assert!(view.refine_at().is_none(), "not rough yet");
        // The rough frame marks itself; its deadline is the motion's
        // end, and once passed the whole-picture resample is due.
        view.rough = true;
        assert_eq!(view.refine_at(), Some(view.motion_until));
        let view = settled(view);
        assert!(matches!(
            view.frame(pw, ph, ceil, Instant::now()),
            Frame::Full
        ));
        // The frame it lands on answers nothing further.
        let mut view = view;
        view.rough = false;
        view.scaled = Some(Sharp {
            w: pw,
            h: ph,
            bytes: vec![0; (pw * ph * 4) as usize],
            spot: None,
        });
        assert!(matches!(
            view.frame(pw, ph, ceil, Instant::now()),
            Frame::Exact
        ));
        assert_eq!(view.refine_at(), None);
    }

    #[test]
    fn past_natural_size_the_picture_is_its_own_sharp_frame() {
        let mut view = drawn(viewer(300, 200), (1000.0, 600.0));
        view.zoom = 1.0;
        view.scaled = None;
        assert!(matches!(
            view.frame(300, 200, 1, Instant::now()),
            Frame::Upscale
        ));
        assert_eq!(view.refine_at(), None);
    }

    #[test]
    fn a_deep_zoom_trims_the_resample_to_the_window() {
        let mut view = settled(drawn(viewer(4000, 3000), (1000.0, 600.0)));
        view.zoom = 0.8;
        // The rect's pixels are past a tiny budget, so the trim wins.
        let pw = (4000.0 * 0.8) as u32;
        let ph = (3000.0 * 0.8) as u32;
        assert!(matches!(
            view.frame(pw, ph, 1000, Instant::now()),
            Frame::Crop
        ));
        let (spot, (tw, th), (sx, sy, sw, sh)) = view.trim(1.0);
        // The trim covers the visible window's worth of natural pixels:
        // the width whole, the height less the caption band.
        assert_eq!(spot.2, 1250, "the window's width over the zoom");
        assert_eq!(spot.3, 696, "the window's height less the caption");
        assert_eq!(tw, (spot.2 as f32 * 0.8).round() as u32);
        assert_eq!(th, (spot.3 as f32 * 0.8).round() as u32);
        // And it blits at its own place on the picture.
        let (x, y, _, _) = view.rect((1000.0, 600.0));
        assert!((sx - (x + spot.0 as f32 * 0.8)).abs() < 1e-6);
        assert!((sy - (y + spot.1 as f32 * 0.8)).abs() < 1e-6);
        assert!((sw - spot.2 as f32 * 0.8).abs() < 1e-6);
        assert!((sh - spot.3 as f32 * 0.8).abs() < 1e-6);
    }

    #[test]
    fn the_trim_stays_inside_the_picture() {
        let mut view = settled(drawn(viewer(1000, 800), (500.0, 400.0)));
        // Zoomed past the window on every side, half off it.
        view.zoom = 2.0;
        view.offset = (-200.0, 100.0);
        let ((nx, ny, nw, nh), _, _) = view.trim(1.0);
        assert!(nw >= 1 && nh >= 1);
        assert!(nx + nw <= 1000 && ny + nh <= 800, "clamped at the edges");
        // The trim is the visible part: the window's worth of natural
        // pixels, wherever the pan put it.
        assert!(
            (nw as f32 - 250.0).abs() <= 1.0,
            "the window's width over the zoom"
        );
        assert!(
            (nh as f32 - 178.0).abs() <= 1.0,
            "its height less the caption"
        );
    }
}

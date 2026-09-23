//! The corner word count: a small standing line at the page's bottom
//! right while the setting is on. It draws in the theme's comment color,
//! the one quiet text color every theme keeps readable, on a plate of
//! the page's own color (`paint::paper`), so the text scrolling under it
//! never crosses its letters. It takes no key and no click.

use std::time::Duration;

use crate::paint::painter::Painter;
use crate::style::fonts::BODY_FAMILY;
use crate::style::theme::{Rgba, Theme};
use crate::ui::scrollbar::STRIP_WIDTH;

/// How long a change rests before the file is counted again, so a burst
/// of typing or a selection drag costs one count at its end.
pub const REST: Duration = Duration::from_millis(150);

pub const SIZE: f32 = 12.0;
const PAD: f32 = 8.0;
const HEIGHT: f32 = 22.0;
const RADIUS: f32 = 6.0;
/// Clear of the scrollbar's strip on the right, close to the bottom.
const RIGHT: f32 = STRIP_WIDTH + 4.0;
const BOTTOM: f32 = 8.0;

/// The plate's left, top and width for a text `text_w` wide in a window
/// `width` by `height`. A window narrower than the line keeps the
/// line's start in view.
pub fn place(text_w: f32, width: f32, height: f32) -> (f32, f32, f32) {
    let w = text_w + 2.0 * PAD;
    let x = (width - w - RIGHT).max(0.0);
    (x, height - HEIGHT - BOTTOM, w)
}

pub fn draw(
    painter: &mut Painter,
    theme: &Theme,
    text: &str,
    paper: Rgba,
    width: f32,
    height: f32,
) {
    let text_w = painter.measure(text, BODY_FAMILY, SIZE, 400);
    let (x, y, w) = place(text_w, width, height);
    painter.fill(x, y, w, HEIGHT, RADIUS, paper);
    painter.text(
        x + PAD,
        y + 3.0,
        text,
        BODY_FAMILY,
        SIZE,
        400,
        theme.syntax.comment,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_sits_in_the_bottom_right_clear_of_the_scrollbar() {
        let (x, y, w) = place(300.0, 1200.0, 800.0);
        assert_eq!(w, 316.0);
        assert_eq!(x + w, 1200.0 - STRIP_WIDTH - 4.0);
        assert_eq!(y + HEIGHT, 800.0 - BOTTOM);
    }

    #[test]
    fn a_narrow_window_keeps_the_start_of_the_line() {
        let (x, _, _) = place(300.0, 200.0, 800.0);
        assert_eq!(x, 0.0);
    }
}

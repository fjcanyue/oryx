//! Images brought into a markdown file: one pasted from the clipboard
//! is written as a PNG into an `images` folder beside the file, one
//! dropped on the window is linked where it lies when it sits under the
//! file's folder and copied into `images` otherwise, so the file and
//! its pictures travel together. Either way the link goes in at the
//! caret, on a line of its own.

use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The folder beside the markdown file that holds its pictures.
pub const FOLDER: &str = "images";

/// The longer side a picture brought in is reduced to, in pixels. A
/// page never draws a picture wider than its window, and a 4800 by 3200
/// photo costs half a second and 130 MB to show where a 2560 one costs
/// a tenth of that (measured 21/09/2026). The limit sits above what a
/// 2560 screen captures, so such a screenshot is never touched and its
/// small text stays sharp.
pub const LIMIT: u32 = 2560;

/// A picture on disk beside the file, and what bringing it in changed.
#[derive(Debug, PartialEq)]
pub struct Saved {
    /// The picture's path from the file's folder.
    pub relative: PathBuf,
    /// The size it had and the size it was reduced to, when it was.
    pub resized: Option<((u32, u32), (u32, u32))>,
}

/// The size a picture is reduced to, its longer side at the limit and
/// its shape kept; None for a picture within the limit.
pub fn fitted(width: u32, height: u32) -> Option<(u32, u32)> {
    let longer = width.max(height);
    if longer <= LIMIT {
        return None;
    }
    let scale = |side: u32| {
        let scaled =
            (u64::from(side) * u64::from(LIMIT) + u64::from(longer) / 2) / u64::from(longer);
        (scaled as u32).max(1)
    };
    Some((scale(width), scale(height)))
}

/// The filter a picture is reduced with.
const FILTER: image::imageops::FilterType = image::imageops::FilterType::CatmullRom;

/// The quality a reduced JPEG is written at.
const JPEG_QUALITY: u8 = 90;

/// Whether a dropped file is a picture a markdown page shows, told by
/// its extension.
pub fn is_image(path: &Path) -> bool {
    const KNOWN: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| KNOWN.iter().any(|known| ext.eq_ignore_ascii_case(known)))
}

/// The name of a pasted picture: the file's own name and the minute,
/// `notes-20260921-1542.png`, so the pictures of one file sort together
/// in a folder several files share. The stem keeps letters, digits,
/// dashes and underscores; anything else becomes a dash, so the link
/// needs no escaping.
pub fn pasted_name(file: &Path, stamp: &str) -> String {
    let stem: String = file
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let stem = if stem.is_empty() { "image" } else { &stem };
    format!("{stem}-{stamp}.png")
}

/// A name nothing in `dir` has yet: the name itself, else the name with
/// `-2`, `-3` before its extension. Two pictures pasted in one minute
/// are the case.
pub fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (name, String::new()),
    };
    (2u32..)
        .map(|n| format!("{stem}-{n}{ext}"))
        .find(|candidate| !dir.join(candidate).exists())
        .unwrap_or_else(|| name.to_string())
}

/// The path of `image` from `dir` when it lies under it, the case of a
/// picture linked where it is.
pub fn under(dir: &Path, image: &Path) -> Option<PathBuf> {
    image.strip_prefix(dir).ok().map(Path::to_path_buf)
}

/// A relative path as a markdown link destination: forward slashes on
/// every platform, and angle brackets around a path holding a space or
/// a parenthesis, which would end a bare destination early.
pub fn destination(relative: &Path) -> String {
    let joined = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if joined.contains([' ', '(', ')']) {
        format!("<{joined}>")
    } else {
        joined
    }
}

/// The edit that links pictures at the caret.
#[derive(Debug, PartialEq)]
pub struct Insert {
    /// What the text replaces: the selection, or nothing at the caret.
    pub replace: Range<usize>,
    pub text: String,
    /// Between the first link's square brackets, where a pasted
    /// picture's description is typed.
    pub inside: usize,
    /// After the last link, where the next dropped picture goes: a drop
    /// of several files delivers them one at a time.
    pub after: usize,
}

/// One `![](destination)` per picture, each on a line of its own: a
/// line break is added before when text stands before the caret on its
/// line, and after when text stands after it. A selection is replaced,
/// as any paste replaces it.
pub fn insertion(source: &str, replace: Range<usize>, destinations: &[String]) -> Insert {
    let start = replace.start.min(source.len());
    let end = replace.end.clamp(start, source.len());
    let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[end..].find('\n').map_or(source.len(), |at| end + at);
    let before = !source[line_start..start].trim().is_empty();
    let after = !source[end..line_end].trim().is_empty();
    let links: Vec<String> = destinations
        .iter()
        .map(|destination| format!("![]({destination})"))
        .collect();
    let mut text = String::new();
    if before {
        text.push('\n');
    }
    let lead = text.len();
    text.push_str(&links.join("\n"));
    let links_end = text.len();
    if after {
        text.push('\n');
    }
    Insert {
        replace: start..end,
        text,
        inside: start + lead + "![".len(),
        after: start + links_end,
    }
}

/// Writes a pasted picture, RGBA rows, as a PNG into the images folder
/// beside `file`, the folder made when missing. A picture above the
/// limit is reduced to it unless `full` asks for every pixel.
pub fn save_pasted(
    file: &Path,
    stamp: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    full: bool,
) -> io::Result<Saved> {
    let picture = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| io::Error::other("the clipboard's picture is cut short"))?;
    let target = fitted(width, height).filter(|_| !full);
    let picture = match target {
        Some((w, h)) => image::imageops::resize(&picture, w, h, FILTER),
        None => picture,
    };
    let dir = folder_of(file);
    let images = dir.join(FOLDER);
    std::fs::create_dir_all(&images)?;
    let name = free_name(&images, &pasted_name(file, stamp));
    picture
        .save_with_format(images.join(&name), image::ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(Saved {
        relative: Path::new(FOLDER).join(name),
        resized: target.map(|to| ((width, height), to)),
    })
}

/// Brings a dropped picture to the file: linked where it lies when it
/// sits under the file's folder, and never touched there; copied into
/// the images folder otherwise. The copy of a JPEG or a PNG above the
/// limit is reduced to it, in its own format and turned the way its
/// camera says; a GIF may move, a WebP may too and an SVG has no
/// pixels, so those are copied byte for byte. The dropped file itself
/// is never changed.
pub fn adopt_dropped(file: &Path, image: &Path) -> io::Result<Saved> {
    let dir = folder_of(file);
    let image = image.canonicalize()?;
    if let Some(relative) = under(&dir, &image) {
        return Ok(Saved {
            relative,
            resized: None,
        });
    }
    let name = image
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| io::Error::other("the picture has no name"))?;
    let images = dir.join(FOLDER);
    std::fs::create_dir_all(&images)?;
    let name = free_name(&images, &name);
    let resized = match reduced(&image) {
        Some(reduced) => {
            reduced.write(&images.join(&name))?;
            Some((reduced.from, reduced.to))
        }
        None => {
            std::fs::copy(&image, images.join(&name))?;
            None
        }
    };
    Ok(Saved {
        relative: Path::new(FOLDER).join(name),
        resized,
    })
}

/// A dropped picture decoded, turned upright and reduced to the limit.
struct Reduced {
    picture: image::DynamicImage,
    format: image::ImageFormat,
    from: (u32, u32),
    to: (u32, u32),
}

impl Reduced {
    fn write(&self, path: &Path) -> io::Result<()> {
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        match self.format {
            image::ImageFormat::Jpeg => {
                image::codecs::jpeg::JpegEncoder::new_with_quality(file, JPEG_QUALITY)
                    .encode_image(&self.picture.to_rgb8())
            }
            _ => self
                .picture
                .write_with_encoder(image::codecs::png::PngEncoder::new(file)),
        }
        .map_err(io::Error::other)
    }
}

/// The reduced form of a picture file, or None when the file is copied
/// as it is: within the limit by its header, not a JPEG or a PNG by its
/// content, or unreadable, which is the copy's business to report.
fn reduced(path: &Path) -> Option<Reduced> {
    use image::ImageDecoder;
    let reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let format = reader
        .format()
        .filter(|f| matches!(f, image::ImageFormat::Jpeg | image::ImageFormat::Png))?;
    let mut decoder = reader.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    fitted(width, height)?;
    // The camera's word on which way is up is lost with the rest of
    // the file's data, so it is applied to the pixels first.
    let orientation = decoder.orientation().ok()?;
    let mut picture = image::DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    let from = (picture.width(), picture.height());
    let to = fitted(from.0, from.1)?;
    Some(Reduced {
        picture: picture.resize_exact(to.0, to.1, FILTER),
        format,
        from,
        to,
    })
}

/// The folder a file's pictures are counted from.
fn folder_of(file: &Path) -> PathBuf {
    file.parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oryx-attach-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn pictures_are_told_by_their_extension() {
        for name in ["a.png", "b.JPG", "c.jpeg", "d.gif", "e.webp", "f.svg"] {
            assert!(is_image(Path::new(name)), "{name}");
        }
        for name in [
            "notes.md",
            "archive.zip",
            "png",
            "photo.png.txt",
            "scan.pdf",
        ] {
            assert!(!is_image(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn a_pasted_picture_is_named_after_the_file_and_the_minute() {
        assert_eq!(
            pasted_name(Path::new("/notes/trip.md"), "20260921-1542"),
            "trip-20260921-1542.png"
        );
        assert_eq!(
            pasted_name(Path::new("/notes/My trip (2026).md"), "20260921-1542"),
            "My-trip--2026--20260921-1542.png",
            "nothing in the name needs escaping in a link"
        );
        assert_eq!(
            pasted_name(Path::new("/notes/été_1.markdown"), "20260921-1542"),
            "été_1-20260921-1542.png",
            "letters of any script stay"
        );
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = scratch("free");
        assert_eq!(free_name(&dir, "trip-1.png"), "trip-1.png");
        std::fs::write(dir.join("trip-1.png"), b"x").unwrap();
        assert_eq!(free_name(&dir, "trip-1.png"), "trip-1-2.png");
        std::fs::write(dir.join("trip-1-2.png"), b"x").unwrap();
        assert_eq!(free_name(&dir, "trip-1.png"), "trip-1-3.png");
        assert_eq!(free_name(&dir.join("missing"), "a.png"), "a.png");
    }

    #[test]
    fn a_picture_under_the_folder_is_found_from_it() {
        let dir = Path::new("/notes");
        assert_eq!(
            under(dir, Path::new("/notes/art/cat.png")),
            Some(PathBuf::from("art/cat.png"))
        );
        assert_eq!(
            under(dir, Path::new("/notes/cat.png")),
            Some(PathBuf::from("cat.png"))
        );
        assert_eq!(under(dir, Path::new("/elsewhere/cat.png")), None);
        assert_eq!(under(dir, Path::new("/notes-old/cat.png")), None);
    }

    #[test]
    fn a_destination_is_safe_inside_a_link() {
        assert_eq!(destination(Path::new("images/cat.png")), "images/cat.png");
        assert_eq!(
            destination(&Path::new("art").join("old").join("cat.png")),
            "art/old/cat.png"
        );
        assert_eq!(destination(Path::new("art/my cat.png")), "<art/my cat.png>");
        assert_eq!(destination(Path::new("art/cat(1).png")), "<art/cat(1).png>");
    }

    #[test]
    fn the_link_takes_a_line_of_its_own() {
        let one = ["images/a.png".to_string()];
        // An empty line: nothing added around.
        let got = insertion("Before.\n\nAfter.\n", 8..8, &one);
        assert_eq!(got.replace, 8..8);
        assert_eq!(got.text, "![](images/a.png)");
        assert_eq!(got.inside, 10, "between the square brackets");
        assert_eq!(got.after, 8 + 17, "after the closing parenthesis");
        // The end of a line of text: a break before.
        let got = insertion("Before.\n", 7..7, &one);
        assert_eq!(got.text, "\n![](images/a.png)");
        assert_eq!(got.inside, 7 + 3);
        assert_eq!(got.after, 7 + 18);
        // The start of a line of text: a break after, the caret before it.
        let got = insertion("Before.\n", 0..0, &one);
        assert_eq!(got.text, "![](images/a.png)\n");
        assert_eq!((got.inside, got.after), (2, 17));
        // The middle of a line: both.
        let got = insertion("Before.\n", 3..3, &one);
        assert_eq!(got.text, "\n![](images/a.png)\n");
        // Spaces alone around the caret are not text.
        let got = insertion("  \n", 1..1, &one);
        assert_eq!(got.text, "![](images/a.png)");
        // An empty file.
        let got = insertion("", 0..0, &one);
        assert_eq!(
            (got.replace, got.text.as_str(), got.inside),
            (0..0, "![](images/a.png)", 2)
        );
    }

    #[test]
    fn a_selection_is_replaced_by_the_link() {
        let one = ["images/a.png".to_string()];
        let got = insertion("Keep this word here.\n", 10..14, &one);
        assert_eq!(got.replace, 10..14);
        assert_eq!(
            got.text, "\n![](images/a.png)\n",
            "text stands on both sides"
        );
        let whole = insertion("old line\nnext\n", 0..8, &one);
        assert_eq!(
            whole.text, "![](images/a.png)",
            "a whole line selected leaves no text around"
        );
    }

    #[test]
    fn several_pictures_take_a_line_each() {
        let two = ["images/a.png".to_string(), "<art/my cat.png>".to_string()];
        let got = insertion("", 0..0, &two);
        assert_eq!(got.text, "![](images/a.png)\n![](<art/my cat.png>)");
        assert_eq!(got.inside, 2, "the first picture's description");
        assert_eq!(got.after, got.text.len());
    }

    #[test]
    fn a_pasted_picture_lands_in_the_images_folder_as_a_png() {
        let dir = scratch("paste");
        let file = dir.join("trip.md");
        let rgba: Vec<u8> = [255u8, 0, 0, 255].repeat(6);
        let first = save_pasted(&file, "20260921-1542", 3, 2, &rgba, false).unwrap();
        assert_eq!(
            first.relative,
            PathBuf::from("images/trip-20260921-1542.png")
        );
        assert_eq!(first.resized, None);
        let written = image::open(dir.join(&first.relative)).unwrap().to_rgba8();
        assert_eq!(written.dimensions(), (3, 2));
        assert_eq!(written.get_pixel(2, 1).0, [255, 0, 0, 255]);
        let second = save_pasted(&file, "20260921-1542", 3, 2, &rgba, false).unwrap();
        assert_eq!(
            second.relative,
            PathBuf::from("images/trip-20260921-1542-2.png"),
            "the same minute keeps both"
        );
        assert!(save_pasted(&file, "20260921-1542", 3, 2, &rgba[..5], false).is_err());
    }

    #[test]
    fn a_dropped_picture_is_linked_in_place_or_copied() {
        let dir = scratch("drop");
        let file = dir.join("trip.md");
        std::fs::create_dir_all(dir.join("art")).unwrap();
        std::fs::write(dir.join("art/cat.png"), b"inside").unwrap();
        assert_eq!(
            adopt_dropped(&file, &dir.join("art/cat.png"))
                .unwrap()
                .relative,
            PathBuf::from("art/cat.png")
        );
        assert!(
            !dir.join(FOLDER).exists(),
            "nothing copied for a picture in place"
        );

        let outside = scratch("drop-outside");
        std::fs::write(outside.join("dog.jpg"), b"outside").unwrap();
        let copied = adopt_dropped(&file, &outside.join("dog.jpg"))
            .unwrap()
            .relative;
        assert_eq!(copied, PathBuf::from("images/dog.jpg"));
        assert_eq!(std::fs::read(dir.join(&copied)).unwrap(), b"outside");
        assert!(outside.join("dog.jpg").exists(), "a copy, not a move");
        let again = adopt_dropped(&file, &outside.join("dog.jpg"))
            .unwrap()
            .relative;
        assert_eq!(
            again,
            PathBuf::from("images/dog-2.jpg"),
            "a taken name is kept"
        );
        assert!(adopt_dropped(&file, &outside.join("missing.png")).is_err());
    }

    #[test]
    fn a_picture_above_the_limit_is_fitted_to_it() {
        assert_eq!(fitted(2560, 1440), None, "a 2560 screen's capture stays");
        assert_eq!(fitted(1440, 2560), None);
        assert_eq!(fitted(100, 100), None);
        assert_eq!(fitted(4800, 3200), Some((2560, 1707)));
        assert_eq!(fitted(3200, 4800), Some((1707, 2560)));
        assert_eq!(fitted(3840, 2160), Some((2560, 1440)));
        assert_eq!(fitted(2561, 10), Some((2560, 10)));
        assert_eq!(fitted(10000, 3), Some((2560, 1)), "never a side of zero");
    }

    /// A wide picture, a gradient so a resize has something to keep.
    fn wide(width: u32, height: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x * 255 / width) as u8, (y * 255 / height) as u8, 90, 255])
        })
    }

    fn size_of(path: &Path) -> (u32, u32) {
        image::image_dimensions(path).unwrap()
    }

    fn format_of(path: &Path) -> image::ImageFormat {
        image::ImageReader::open(path)
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .format()
            .unwrap()
    }

    #[test]
    fn a_big_pasted_picture_is_reduced_unless_every_pixel_is_asked_for() {
        let dir = scratch("paste-big");
        let file = dir.join("trip.md");
        let picture = wide(3000, 120);
        let stamp = "20260921-1700";
        let reduced = save_pasted(&file, stamp, 3000, 120, picture.as_raw(), false).unwrap();
        assert_eq!(reduced.resized, Some(((3000, 120), (2560, 102))));
        assert_eq!(size_of(&dir.join(&reduced.relative)), (2560, 102));
        let whole = save_pasted(&file, stamp, 3000, 120, picture.as_raw(), true).unwrap();
        assert_eq!(whole.resized, None);
        assert_eq!(size_of(&dir.join(&whole.relative)), (3000, 120));
    }

    #[test]
    fn a_big_dropped_picture_is_copied_reduced_in_its_own_format() {
        let dir = scratch("drop-big");
        let file = dir.join("trip.md");
        let outside = scratch("drop-big-outside");
        let picture = image::DynamicImage::ImageRgba8(wide(3000, 120));
        picture.save(outside.join("wide.png")).unwrap();
        picture.to_rgb8().save(outside.join("wide.jpg")).unwrap();
        for (name, format) in [
            ("wide.png", image::ImageFormat::Png),
            ("wide.jpg", image::ImageFormat::Jpeg),
        ] {
            let before = std::fs::read(outside.join(name)).unwrap();
            let saved = adopt_dropped(&file, &outside.join(name)).unwrap();
            assert_eq!(saved.relative, Path::new(FOLDER).join(name));
            assert_eq!(saved.resized, Some(((3000, 120), (2560, 102))), "{name}");
            assert_eq!(size_of(&dir.join(&saved.relative)), (2560, 102));
            assert_eq!(format_of(&dir.join(&saved.relative)), format);
            assert_eq!(
                std::fs::read(outside.join(name)).unwrap(),
                before,
                "the dropped file itself is never changed"
            );
        }
    }

    #[test]
    fn what_is_small_or_may_move_is_copied_byte_for_byte() {
        let dir = scratch("drop-as-is");
        let file = dir.join("trip.md");
        let outside = scratch("drop-as-is-outside");
        let small = image::DynamicImage::ImageRgba8(wide(800, 60));
        small.to_rgb8().save(outside.join("small.jpg")).unwrap();
        let big = image::DynamicImage::ImageRgba8(wide(3000, 120));
        big.save_with_format(outside.join("big.gif"), image::ImageFormat::Gif)
            .unwrap();
        big.save_with_format(outside.join("big.webp"), image::ImageFormat::WebP)
            .unwrap();
        std::fs::write(
            outside.join("big.svg"),
            b"<svg width=\"9000\" height=\"9000\"/>",
        )
        .unwrap();
        std::fs::write(outside.join("broken.png"), b"not a picture at all").unwrap();
        for name in ["small.jpg", "big.gif", "big.webp", "big.svg", "broken.png"] {
            let saved = adopt_dropped(&file, &outside.join(name)).unwrap();
            assert_eq!(saved.resized, None, "{name}");
            assert_eq!(
                std::fs::read(dir.join(&saved.relative)).unwrap(),
                std::fs::read(outside.join(name)).unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn a_big_picture_already_under_the_folder_is_never_touched() {
        let dir = scratch("drop-in-place-big");
        let file = dir.join("trip.md");
        let picture = image::DynamicImage::ImageRgba8(wide(3000, 120));
        picture.save(dir.join("wide.png")).unwrap();
        let before = std::fs::read(dir.join("wide.png")).unwrap();
        let saved = adopt_dropped(&file, &dir.join("wide.png")).unwrap();
        assert_eq!(saved.relative, PathBuf::from("wide.png"));
        assert_eq!(saved.resized, None);
        assert_eq!(std::fs::read(dir.join("wide.png")).unwrap(), before);
        assert!(!dir.join(FOLDER).exists());
    }

    #[test]
    fn a_photo_is_turned_the_way_its_camera_says_before_it_is_reduced() {
        let dir = scratch("drop-turned");
        let file = dir.join("trip.md");
        let outside = scratch("drop-turned-outside");
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgba8(wide(3000, 120))
            .to_rgb8()
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        // An Exif block right after the start marker, one entry: the
        // orientation tag (0x0112) at 6, a quarter turn clockwise.
        let mut exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0".to_vec();
        exif.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
        let mut turned = jpeg[..2].to_vec();
        turned.extend_from_slice(&[0xFF, 0xE1]);
        turned.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        turned.extend_from_slice(&exif);
        turned.extend_from_slice(&jpeg[2..]);
        std::fs::write(outside.join("phone.jpg"), &turned).unwrap();
        let saved = adopt_dropped(&file, &outside.join("phone.jpg")).unwrap();
        assert_eq!(
            size_of(&dir.join(&saved.relative)),
            (102, 2560),
            "upright, since the copy no longer says how to turn it"
        );
        assert_eq!(saved.resized, Some(((120, 3000), (102, 2560))));
    }
}

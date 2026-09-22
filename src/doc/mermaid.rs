//! The Mermaid adapter: the one module that knows the third-party
//! renderer. Markdown only produces `BlockKind::Mermaid`; here the
//! diagram source becomes an svg document with its natural size, and
//! every third-party error becomes one of Oryx's own. Nothing else in
//! Oryx names `mermaid_rs_renderer`.

use std::sync::Arc;

use crate::style::theme::{hex_string, Rgba, Theme};

/// Oryx's Mermaid error. The renderer's own error type never crosses
/// this module; `Parse` and `Render` carry its message, which reading
/// mode shows inside the error placeholder.
#[derive(Debug, PartialEq)]
pub enum MermaidError {
    /// The fence held nothing but whitespace.
    Empty,
    /// The renderer rejected the source.
    Parse(String),
    /// The renderer failed past parsing.
    Render(String),
    /// The output did not read as an svg document.
    InvalidSvg(String),
    /// The output carried no usable pixel size.
    InvalidDimensions,
}

impl std::fmt::Display for MermaidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MermaidError::Empty => f.write_str("the diagram is empty"),
            MermaidError::Parse(msg) => write!(f, "{msg}"),
            MermaidError::Render(msg) => write!(f, "{msg}"),
            MermaidError::InvalidSvg(msg) => write!(f, "invalid svg output: {msg}"),
            MermaidError::InvalidDimensions => f.write_str("the diagram has no size"),
        }
    }
}

/// The colors a diagram draws with, one role per reading surface.
/// `Default` holds the renderer's own classic palette; the theme
/// mapping lands in a later phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MermaidTheme {
    pub background: Rgba,
    pub foreground: Rgba,
    pub primary: Rgba,
    pub border: Rgba,
    pub line: Rgba,
    pub accent: Rgba,
}

impl Default for MermaidTheme {
    fn default() -> Self {
        MermaidTheme {
            background: Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            foreground: Rgba {
                r: 51,
                g: 51,
                b: 51,
                a: 255,
            },
            primary: Rgba {
                r: 0xEC,
                g: 0xEC,
                b: 0xFF,
                a: 255,
            },
            border: Rgba {
                r: 0x7B,
                g: 0x88,
                b: 0xA8,
                a: 255,
            },
            line: Rgba {
                r: 0x2F,
                g: 0x3B,
                b: 0x4D,
                a: 255,
            },
            accent: Rgba {
                r: 0xFF,
                g: 0xFF,
                b: 0xDE,
                a: 255,
            },
        }
    }
}

impl MermaidTheme {
    /// The reading theme's palette as diagram roles: the page behind,
    /// the body text, the code panel and its border, the rule line, and
    /// the link accent. An adapter, not Theme fields: Mermaid owns
    /// nothing in the core palette.
    pub fn from_oryx(theme: &Theme) -> Self {
        MermaidTheme {
            background: theme.surface.background,
            foreground: theme.text.body,
            primary: theme.blocks.code_bg,
            border: theme.blocks.code_border,
            line: theme.surface.foreground,
            accent: theme.text.link,
        }
    }

    /// The renderer's theme: its classic palette with this theme's
    /// roles laid over it, everything the six roles miss left as the
    /// renderer intends.
    fn third_party(&self) -> mermaid_rs_renderer::Theme {
        let mut theme = mermaid_rs_renderer::Theme::mermaid_default();
        theme.background = hex_string(self.background);
        theme.text_color = hex_string(self.foreground);
        theme.primary_text_color = hex_string(self.foreground);
        theme.primary_color = hex_string(self.primary);
        theme.primary_border_color = hex_string(self.border);
        theme.line_color = hex_string(self.line);
        theme.secondary_color = hex_string(self.accent);
        theme.cluster_background = hex_string(self.primary);
        theme.cluster_border = hex_string(self.border);
        theme
    }
}

/// A rendered diagram: the svg document and its natural pixel size.
#[derive(Debug, Clone)]
pub struct MermaidRender {
    pub svg: Arc<[u8]>,
    pub width: f32,
    pub height: f32,
}

/// The cache namespace version: bump it when renderer behavior changes
/// and every old entry misses.
pub const CACHE_VERSION: &str = "oryx-mermaid-v1";

/// A diagram's identity in the media cache: the version, the source,
/// and the theme fingerprint hashed together. A typed key, not a bare
/// String: same source under a different theme must not hit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MermaidCacheKey(String);

impl MermaidCacheKey {
    /// The media-cache address the diagram registers under.
    pub fn uri(&self) -> String {
        format!("mermaid://{}", self.0)
    }
}

/// FNV-1a, the hash the fetch cache already keys by, fed the version,
/// the theme's six roles, then the source.
pub fn cache_key(source: &str, theme: &MermaidTheme) -> MermaidCacheKey {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(CACHE_VERSION.as_bytes());
    for color in [
        theme.background,
        theme.foreground,
        theme.primary,
        theme.border,
        theme.line,
        theme.accent,
    ] {
        feed(&[color.r, color.g, color.b, color.a]);
    }
    feed(source.as_bytes());
    MermaidCacheKey(format!("{hash:016x}"))
}

/// Renders diagram source into an svg with its natural size. The
/// source arrives trimmed by the caller or here; empty never reaches
/// the renderer.
pub fn render(source: &str, theme: &MermaidTheme) -> Result<MermaidRender, MermaidError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MermaidError::Empty);
    }
    let options = mermaid_rs_renderer::RenderOptions {
        theme: theme.third_party(),
        layout: mermaid_rs_renderer::LayoutConfig::default(),
    };
    let svg = mermaid_rs_renderer::render_with_options(source, options).map_err(|err| {
        if err
            .downcast_ref::<mermaid_rs_renderer::ParseError>()
            .is_some()
        {
            MermaidError::Parse(err.to_string())
        } else {
            MermaidError::Render(err.to_string())
        }
    })?;
    let (width, height) = dimensions(&svg)?;
    let key = cache_key(source, theme);
    dump_debug(&key, svg.as_bytes());
    Ok(MermaidRender {
        svg: Arc::from(svg.into_bytes()),
        width,
        height,
    })
}

/// Writes a rendered svg under `target/mermaid-debug/` when
/// `ORYX_DUMP_MERMAID` is set — the one debug affordance, for renderer
/// comparison and regression digging; unset, which every normal run is,
/// the product path stays pure memory. `ORYX_DUMP_MERMAID_DIR` moves
/// the output (the migration's old-renderer baseline lives in
/// `target/mermaid-baseline/`).
fn dump_debug(key: &MermaidCacheKey, svg: &[u8]) {
    if std::env::var_os("ORYX_DUMP_MERMAID").is_none() {
        return;
    }
    let dir = std::env::var_os("ORYX_DUMP_MERMAID_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target").join("mermaid-debug"));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{}.svg", key.0)), svg);
}

/// The svg's natural size from its own root: the viewBox first, the
/// one attribute always written in pixels (several diagram kinds write
/// `width="100%"` instead), then absolute width/height attributes. A
/// size that is not a finite positive number is no size.
fn dimensions(svg: &str) -> Result<(f32, f32), MermaidError> {
    let invalid = |msg: &str| MermaidError::InvalidSvg(msg.to_string());
    let doc = roxmltree::Document::parse(svg).map_err(|e| invalid(&e.to_string()))?;
    let root = doc.root_element();
    if !root.is_element() || root.tag_name().name() != "svg" {
        return Err(invalid("the root node is not svg"));
    }
    let pair = |w: f32, h: f32| {
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            Ok((w, h))
        } else {
            Err(MermaidError::InvalidDimensions)
        }
    };
    if let Some(view_box) = root.attribute("viewBox") {
        let nums: Vec<Option<f32>> = view_box
            .split_whitespace()
            .map(|value| value.parse::<f32>().ok())
            .collect();
        if let [Some(_), Some(_), Some(w), Some(h)] = nums[..] {
            return pair(w, h);
        }
    }
    let absolute = |value: &str| value.parse::<f32>().ok().filter(|n| n.is_finite());
    match (root.attribute("width"), root.attribute("height")) {
        (Some(w), Some(h)) if absolute(w).is_some() && absolute(h).is_some() => pair(
            absolute(w).unwrap_or_default(),
            absolute(h).unwrap_or_default(),
        ),
        _ => Err(MermaidError::InvalidDimensions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(source: &str) -> MermaidRender {
        render(source, &MermaidTheme::default())
            .unwrap_or_else(|err| panic!("a valid diagram renders: {err}"))
    }

    #[test]
    fn a_flowchart_renders_with_finite_dimensions() {
        let out = rendered("flowchart LR\n    A[Start] --> B{Valid?}\n    B -->|Yes| C[Continue]");
        assert!(out.svg.starts_with(b"<svg"), "an svg document");
        assert!(out.width > 0.0 && out.width.is_finite());
        assert!(out.height > 0.0 && out.height.is_finite());
    }

    #[test]
    fn a_sequence_diagram_renders() {
        let out = rendered("sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn a_class_diagram_renders() {
        let out = rendered("classDiagram\n    Animal <|-- Duck");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn a_state_diagram_renders() {
        let out = rendered("stateDiagram-v2\n    [*] --> Idle\n    Idle --> Running");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn an_er_diagram_renders() {
        let out = rendered("erDiagram\n    USER ||--o{ ORDER : places");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn chinese_labels_render() {
        let out = rendered("flowchart LR\n    A[开始] --> B[处理数据]\n    B --> C[完成]");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn a_pie_answers_dimensions_despite_percent_width() {
        // Pie writes width="100%"; the viewBox still carries the size.
        let out = rendered("pie title Tasks\n    \"Done\" : 70\n    \"Todo\" : 30");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn a_mindmap_renders() {
        let out = rendered("mindmap\n  root((Oryx))\n    Read\n    Edit");
        assert!(out.width > 0.0 && out.height > 0.0);
    }

    #[test]
    fn empty_source_is_rejected_before_the_renderer() {
        assert_eq!(
            render("", &MermaidTheme::default()).unwrap_err(),
            MermaidError::Empty
        );
        assert_eq!(
            render("  \n\t", &MermaidTheme::default()).unwrap_err(),
            MermaidError::Empty
        );
    }

    #[test]
    fn a_syntax_error_becomes_a_parse_error() {
        // The parser is lenient with stray tokens, so the error needs a
        // construct it provably rejects: a subgraph that never closes.
        let err = render(
            "flowchart LR\n    subgraph X\n    A --> B",
            &MermaidTheme::default(),
        )
        .expect_err("an unclosed subgraph fails");
        assert!(
            matches!(err, MermaidError::Parse(_) | MermaidError::Render(_)),
            "a renderer failure, got {err:?}"
        );
        assert!(
            err.to_string().to_lowercase().contains("subgraph"),
            "the message names the problem: {err}"
        );
    }

    #[test]
    fn an_unknown_diagram_kind_fails_without_panic() {
        let err = render("notadiagram\n  A --> B", &MermaidTheme::default())
            .expect_err("an unknown kind fails");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn the_theme_roles_reach_the_svg() {
        let theme = MermaidTheme {
            line: Rgba {
                r: 0xAB,
                g: 0xCD,
                b: 0xEF,
                a: 255,
            },
            ..MermaidTheme::default()
        };
        let out = render("flowchart LR\n    A --> B", &theme).expect("renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        assert!(svg.contains("#ABCDEF"), "the line role paints");
    }

    /// A reading theme's roles land one-to-one in the diagram palette.
    #[test]
    fn the_reading_theme_maps_to_the_diagram_roles() {
        let theme = Theme::default_dark();
        let mapped = MermaidTheme::from_oryx(&theme);
        assert_eq!(mapped.background, theme.surface.background);
        assert_eq!(mapped.foreground, theme.text.body);
        assert_eq!(mapped.primary, theme.blocks.code_bg);
        assert_eq!(mapped.border, theme.blocks.code_border);
        assert_eq!(mapped.line, theme.surface.foreground);
        assert_eq!(mapped.accent, theme.text.link);
    }

    /// A light palette good enough to read, the readable counterpart
    /// to the compiled-in dark one.
    fn light_theme() -> Theme {
        let mut theme = Theme::default_dark();
        theme.surface.background = Rgba {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 255,
        };
        theme.surface.foreground = Rgba {
            r: 0x18,
            g: 0x18,
            b: 0x1D,
            a: 255,
        };
        theme.text.body = Rgba {
            r: 0x18,
            g: 0x18,
            b: 0x1D,
            a: 255,
        };
        theme.text.link = Rgba {
            r: 0x0B,
            g: 0x5C,
            b: 0xB8,
            a: 255,
        };
        theme.blocks.code_bg = Rgba {
            r: 0xF3,
            g: 0xF4,
            b: 0xF8,
            a: 255,
        };
        theme.blocks.code_border = Rgba {
            r: 0xC5,
            g: 0xC9,
            b: 0xD4,
            a: 255,
        };
        theme
    }

    /// Both palettes paint and hold reading contrast: the node text
    /// stands off its fill, the lines are visible against the ground.
    #[test]
    fn light_and_dark_diagrams_paint_readable_roles() {
        for theme in [light_theme(), Theme::default_dark()] {
            let palette = MermaidTheme::from_oryx(&theme);
            let out = render("flowchart LR\n    A --> B", &palette)
                .unwrap_or_else(|err| panic!("renders under the theme: {err}"));
            let svg = std::str::from_utf8(&out.svg).unwrap();
            assert!(
                svg.contains(&hex_string(palette.line)),
                "the line role paints"
            );
            assert!(
                svg.contains(&hex_string(palette.primary)),
                "the node fill paints"
            );
            assert!(
                svg.contains(&hex_string(palette.foreground)),
                "the node text paints"
            );
            assert!(
                crate::style::theme::contrast(palette.foreground, palette.primary) >= 3.0,
                "node text keeps contrast against its fill"
            );
            assert!(
                crate::style::theme::contrast(palette.line, palette.background) >= 3.0,
                "lines keep contrast against the ground"
            );
        }
    }

    /// Same source, different theme, different key: the switch can
    /// never serve the old palette.
    #[test]
    fn a_theme_change_misses_the_cache() {
        let source = "flowchart LR\n    A --> B";
        let dark = cache_key(source, &MermaidTheme::from_oryx(&Theme::default_dark()));
        let light = cache_key(source, &MermaidTheme::from_oryx(&light_theme()));
        assert_ne!(dark, light);
        assert_eq!(
            dark,
            cache_key(source, &MermaidTheme::from_oryx(&Theme::default_dark())),
            "the same theme keys stably"
        );
    }
}

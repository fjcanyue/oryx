//! The Mermaid adapter: the one module that knows the third-party
//! renderer. Markdown only produces `BlockKind::Mermaid`; here the
//! diagram source becomes an svg document with its natural size, and
//! every third-party error becomes one of Oryx's own. Nothing else in
//! Oryx names `merman`.

use std::sync::Arc;

use merman::render::{
    HeadlessError, HeadlessRenderer, HostThemeAppearance, HostThemeOutput, HostThemeProfile,
    HostThemeRoles,
};

use crate::doc::mermaid_theme::MermaidAppearance;
use crate::style::fonts::BODY_FAMILY;
use crate::style::theme::hex_string;

pub use crate::doc::mermaid_theme::MermaidPresentation;

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

/// The presentation as Merman's host theme: the palette's roles carry
/// the host-level contract Merman derives its long tail from, the
/// presentation's explicit theme variables pin the contract variables
/// over those derivations, and the family config rides along as site
/// config. The appearance the derivations key on is the palette's own;
/// the diagram font is the body font resvg's generic families already
/// resolve to; the output is the resvg-safe editor preset, because
/// Oryx's consumer of the svg is resvg/usvg, not a browser.
fn host_profile(presentation: &MermaidPresentation) -> HostThemeProfile {
    let palette = &presentation.palette;
    HostThemeProfile {
        appearance: match palette.appearance {
            MermaidAppearance::Light => HostThemeAppearance::Light,
            MermaidAppearance::Dark => HostThemeAppearance::Dark,
        },
        font_family: Some(format!("\"{BODY_FAMILY}\", sans-serif")),
        roles: HostThemeRoles {
            canvas: Some(hex_string(palette.canvas)),
            surface: Some(hex_string(palette.surface)),
            surface_alt: Some(hex_string(palette.surface_alt)),
            surface_muted: Some(hex_string(palette.surface_muted)),
            text: Some(hex_string(palette.text)),
            subtle_text: Some(hex_string(palette.muted_text)),
            border: Some(hex_string(palette.border)),
            line: Some(hex_string(palette.line)),
            edge_label_background: Some(hex_string(palette.label_surface)),
            cluster_background: Some(hex_string(palette.surface_muted)),
            cluster_border: Some(hex_string(palette.border)),
            note_background: Some(hex_string(palette.surface_alt)),
            note_border: Some(hex_string(palette.border)),
            note_text: Some(hex_string(palette.text)),
            actor_background: Some(hex_string(palette.surface)),
            actor_border: Some(hex_string(palette.border)),
            actor_text: Some(hex_string(palette.text)),
            activation_background: Some(hex_string(palette.surface_alt)),
            activation_border: Some(hex_string(palette.border)),
            error: Some(hex_string(palette.danger)),
            warning: Some(hex_string(palette.warning)),
            success: Some(hex_string(palette.success)),
        },
        series_palette: palette.series.iter().map(|color| hex_string(*color)).collect(),
        output: HostThemeOutput::resvg_safe_editor(),
        theme_variables: presentation.theme_variables.to_json(),
        site_config: presentation.family_config.clone(),
        ..HostThemeProfile::default()
    }
}

/// A rendered diagram: the svg document and its natural pixel size.
#[derive(Debug, Clone)]
pub struct MermaidRender {
    pub svg: Arc<[u8]>,
    pub width: f32,
    pub height: f32,
}

/// The renderer identity: bump it when the pinned Merman version or
/// its output behavior changes and every old entry misses.
pub const RENDERER_VERSION: &str = "oryx-merman-0.7-v1";

/// The theme policy identity: bump it when the palette projection or
/// the theme-variable mapping changes, so every old entry misses. The
/// font is part of the policy (one body family, one preset pipeline),
/// so it needs no separate seat in the key.
pub const THEME_POLICY_VERSION: &str = "oryx-merman-theme-v3";

/// A diagram's identity in the media cache: the versions, the source,
/// and the theme fingerprint hashed together. A typed key, not a bare
/// String: same source under a different theme must not hit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MermaidCacheKey(String);

impl MermaidCacheKey {
    /// The media-cache address the diagram registers under.
    pub fn uri(&self) -> String {
        format!("mermaid://{}", self.0)
    }

    /// The stable svg id a diagram renders under: deterministic, easy
    /// to grep in debug dumps, and safe when several diagrams share a
    /// host document's id space (`<defs>`, accessibility ids).
    pub fn svg_id(&self) -> String {
        format!("oryx-mermaid-{}", self.0)
    }
}

/// FNV-1a, the hash the fetch cache already keys by, fed the renderer
/// and policy versions, the whole semantic palette (the theme
/// variables are a deterministic projection of it, so hashing the map
/// too would hash the same colors twice), then the source. Only the
/// palette salts, not the whole reading theme — retinting the sidebar
/// must not re-render every diagram.
pub fn cache_key(source: &str, presentation: &MermaidPresentation) -> MermaidCacheKey {
    let palette = &presentation.palette;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(RENDERER_VERSION.as_bytes());
    feed(THEME_POLICY_VERSION.as_bytes());
    feed(&[match palette.appearance {
        MermaidAppearance::Light => 0,
        MermaidAppearance::Dark => 1,
    }]);
    for color in [
        palette.canvas,
        palette.text,
        palette.muted_text,
        palette.surface,
        palette.surface_alt,
        palette.surface_muted,
        palette.label_surface,
        palette.border,
        palette.line,
        palette.accent,
        palette.info,
        palette.success,
        palette.warning,
        palette.danger,
    ]
    .into_iter()
    .chain(palette.series)
    .chain(palette.on_series)
    {
        feed(&[color.r, color.g, color.b, color.a]);
    }
    feed(source.as_bytes());
    MermaidCacheKey(format!("{hash:016x}"))
}

/// Renders diagram source into an svg with its natural size. The
/// source arrives trimmed by the caller or here; empty never reaches
/// the renderer. Strict parsing, so a bad diagram lands in the error
/// panel instead of a guessed-at picture; the vendored text metrics
/// Mermaid browsers run on; the presentation's resvg-safe editor
/// output, because Oryx's consumer of the svg is resvg/usvg — the
/// profile carries the pipeline, so the plain render call already runs
/// it.
pub fn render(source: &str, presentation: &MermaidPresentation) -> Result<MermaidRender, MermaidError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MermaidError::Empty);
    }
    let key = cache_key(source, presentation);
    let renderer = HeadlessRenderer::new()
        .with_strict_parsing()
        .with_host_theme(&host_profile(presentation))
        .with_vendored_text_measurer()
        .with_diagram_id(&key.svg_id());
    let svg = renderer
        .render_svg_sync(source)
        .map_err(headless_error)?
        .ok_or_else(|| MermaidError::Parse("no Mermaid diagram detected".to_string()))?;
    let (width, height) = dimensions(&svg)?;
    dump_debug(&key, svg.as_bytes());
    Ok(MermaidRender {
        svg: Arc::from(svg.into_bytes()),
        width,
        height,
    })
}

/// Merman's error, classified once: parse failures read as the user's
/// syntax to fix, everything past parsing as the renderer's own
/// trouble. The messages carry over verbatim; no deep matching of the
/// internals — the panel only needs a readable story.
fn headless_error(err: HeadlessError) -> MermaidError {
    match err {
        HeadlessError::Parse(err) => MermaidError::Parse(err.to_string()),
        HeadlessError::Render(err) => MermaidError::Render(err.to_string()),
    }
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
    use crate::doc::mermaid_theme::SERIES_SLOTS;
    use crate::style::theme::{parse_hex, Rgba, Theme};

    fn rendered(source: &str) -> MermaidRender {
        render(source, &light_presentation())
            .unwrap_or_else(|err| panic!("a valid diagram renders: {err}"))
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

    fn light_presentation() -> MermaidPresentation {
        MermaidPresentation::from_oryx(&light_theme())
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
            render("", &light_presentation()).unwrap_err(),
            MermaidError::Empty
        );
        assert_eq!(
            render("  \n\t", &light_presentation()).unwrap_err(),
            MermaidError::Empty
        );
    }

    #[test]
    fn a_syntax_error_becomes_a_parse_error() {
        // Strict parsing rejects an unclosed subgraph outright: the
        // message reads as EOF where the closing `end` was expected.
        let err = render(
            "flowchart LR\n    subgraph X\n    A --> B",
            &light_presentation(),
        )
        .expect_err("an unclosed subgraph fails");
        assert!(
            matches!(err, MermaidError::Parse(_) | MermaidError::Render(_)),
            "a renderer failure, got {err:?}"
        );
        assert!(
            err.to_string().to_lowercase().contains("end"),
            "the message names the problem: {err}"
        );
    }

    #[test]
    fn an_unknown_diagram_kind_fails_without_panic() {
        let err = render("notadiagram\n  A --> B", &light_presentation())
            .expect_err("an unknown kind fails");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn the_theme_roles_reach_the_svg() {
        let mut theme = light_theme();
        theme.blocks.rule = parse_hex("#ABCDEF").unwrap();
        let presentation = MermaidPresentation::from_oryx(&theme);
        let out = render("flowchart LR\n    A --> B", &presentation).expect("renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        assert!(
            svg.contains(&hex_string(presentation.palette.line)),
            "the line role paints"
        );
    }

    /// Both palettes paint and hold reading contrast: the node text
    /// stands off its fill, the lines are visible against the ground.
    #[test]
    fn light_and_dark_diagrams_paint_readable_roles() {
        for theme in [light_theme(), Theme::default_dark()] {
            let presentation = MermaidPresentation::from_oryx(&theme);
            presentation
                .palette
                .validate()
                .expect("the palette holds reading contrast");
            let out = render("flowchart LR\n    A --> B", &presentation)
                .unwrap_or_else(|err| panic!("renders under the theme: {err}"));
            let svg = std::str::from_utf8(&out.svg).unwrap();
            let palette = &presentation.palette;
            assert!(
                svg.contains(&hex_string(palette.line)),
                "the line role paints"
            );
            assert!(
                svg.contains(&hex_string(palette.surface)),
                "the node fill paints"
            );
            assert!(
                svg.contains(&hex_string(palette.text)),
                "the node text paints"
            );
        }
    }

    /// Same source, different theme, different key: the switch can
    /// never serve the old palette.
    #[test]
    fn a_theme_change_misses_the_cache() {
        let source = "flowchart LR\n    A --> B";
        let dark = cache_key(source, &MermaidPresentation::from_oryx(&Theme::default_dark()));
        let light = cache_key(source, &light_presentation());
        assert_ne!(dark, light);
        assert_eq!(
            dark,
            cache_key(
                source,
                &MermaidPresentation::from_oryx(&Theme::default_dark())
            ),
            "the same theme keys stably"
        );
    }

    /// The host profile carries more than colors: the appearance read
    /// off the ground, the body font resvg's generic families resolve
    /// to, the canvas root background of the resvg-safe editor output,
    /// the label ground, the full series, and the explicit theme
    /// variables overriding the role derivations.
    #[test]
    fn the_host_profile_carries_font_output_and_appearance() {
        use merman::render::HostThemeRootBackground;
        let dark = MermaidPresentation::from_oryx(&Theme::default_dark());
        let profile = host_profile(&dark);
        assert_eq!(profile.appearance, HostThemeAppearance::Dark);
        assert_eq!(
            profile.font_family.as_deref(),
            Some("\"DejaVu Sans\", sans-serif")
        );
        assert_eq!(
            profile.output.root_background,
            HostThemeRootBackground::Canvas
        );
        assert_eq!(
            profile.roles.edge_label_background,
            Some(hex_string(dark.palette.label_surface))
        );
        assert_eq!(profile.series_palette.len(), SERIES_SLOTS);
        assert_eq!(
            profile
                .theme_variables
                .get("tertiaryColor")
                .and_then(serde_json::Value::as_str),
            Some(hex_string(dark.palette.label_surface).as_str()),
            "the explicit projection overrides the role derivation"
        );
        let light = light_presentation();
        assert_eq!(host_profile(&light).appearance, HostThemeAppearance::Light);
    }

    /// Both palettes rasterize through Oryx's own decode path, and the
    /// resvg-safe editor output paints the canvas behind the diagram:
    /// the corner pixel answers in the ground color, opaque, never
    /// transparency the reading surface would show through.
    #[test]
    fn light_and_dark_diagrams_rasterize_on_a_painted_ground() {
        for theme in [light_theme(), Theme::default_dark()] {
            let presentation = MermaidPresentation::from_oryx(&theme);
            let out = render("flowchart LR\n    A --> B", &presentation)
                .unwrap_or_else(|err| panic!("renders under the theme: {err}"));
            let svg = std::str::from_utf8(&out.svg).unwrap();
            assert!(svg.contains(BODY_FAMILY), "the body font paints");
            let pixels = crate::doc::images::decode(&out.svg)
                .expect("Oryx's own raster path accepts the svg");
            let corner = pixels.get_pixel(0, 0);
            let ground = presentation.palette.canvas;
            for (got, want) in [
                (corner[0], ground.r),
                (corner[1], ground.g),
                (corner[2], ground.b),
            ] {
                assert!(
                    (i16::from(got) - i16::from(want)).abs() <= 2,
                    "the corner paints the ground: {got} against {want}"
                );
            }
            assert_eq!(corner[3], 255, "the ground is opaque");
        }
    }

    /// The neon regression theme from the design: every accent role is
    /// eye-searing green, the structural roles stay neutral.
    fn neon_presentation(dark: bool) -> MermaidPresentation {
        let mut theme = if dark {
            Theme::default_dark()
        } else {
            light_theme()
        };
        let green = parse_hex("#00FF00").unwrap();
        theme.text.link = green;
        theme.syntax.function = green;
        theme.syntax.keyword = green;
        theme.alerts.tip = green;
        MermaidPresentation::from_oryx(&theme)
    }

    /// The ER bug that started the redesign: the relationship label
    /// `contains` painted the accent green because the old host
    /// profile fed `surface_alt = accent` into `tertiaryColor`. The
    /// label box now paints the label surface, the entities the
    /// surface, and no green appears anywhere in a structural figure.
    #[test]
    fn er_relationship_labels_paint_the_label_surface() {
        for dark in [false, true] {
            let presentation = neon_presentation(dark);
            let out = render(
                "erDiagram\n    TABLE_A ||--o{ TABLE_B : contains",
                &presentation,
            )
            .expect("the ER diagram renders");
            let svg = std::str::from_utf8(&out.svg).unwrap();
            let palette = &presentation.palette;
            assert!(
                svg.contains(".relationshipLabelBox"),
                "the renderer writes the fixed selector"
            );
            assert!(
                svg.contains(&format!(
                    ".relationshipLabelBox{{fill:{}",
                    hex_string(palette.label_surface)
                )),
                "the relationship label paints the label surface"
            );
            assert!(
                svg.contains(&format!(".entityBox{{fill:{}", hex_string(palette.surface))),
                "the entity paints the surface"
            );
            assert!(
                !svg.contains("#00FF00") && !svg.contains("#00ff00"),
                "the neon accent never paints an ER figure (dark={dark})"
            );
        }
    }

    /// The sequence twin of the same bug: the actor box painted the
    /// accent. Actors are boxes — the surface.
    #[test]
    fn sequence_actors_paint_the_surface() {
        for dark in [false, true] {
            let presentation = neon_presentation(dark);
            let out = render(
                "sequenceDiagram\n    Alice->>Bob: Hello",
                &presentation,
            )
            .expect("the sequence diagram renders");
            let svg = std::str::from_utf8(&out.svg).unwrap();
            let palette = &presentation.palette;
            assert!(
                svg.contains(&format!("fill:{}", hex_string(palette.surface))),
                "an actor box fills with the surface"
            );
            assert!(
                !svg.contains("#00FF00") && !svg.contains("#00ff00"),
                "the neon accent never paints a sequence figure (dark={dark})"
            );
        }
    }

    /// Requirement holds the same contract: boxes on the surface,
    /// relation labels on the label ground.
    #[test]
    fn requirement_boxes_and_relation_labels_hold_the_contract() {
        let presentation = neon_presentation(true);
        let source = "requirementDiagram\n    requirement req {\n        id: 1\n        text: the requirement\n        risk: high\n        verifymethod: analysis\n    }\n    element entity {\n        type: simulation\n    }\n    entity - satisfies -> req";
        let out = render(source, &presentation).expect("the requirement diagram renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        let palette = &presentation.palette;
        assert!(
            svg.contains(&format!(".reqBox{{fill:{}", hex_string(palette.surface))),
            "the requirement box paints the surface"
        );
        assert!(
            svg.contains(&format!(".reqLabelBox{{fill:{}", hex_string(palette.label_surface))),
            "the relation label paints the label surface"
        );
        assert!(
            !svg.contains("#00FF00") && !svg.contains("#00ff00"),
            "the neon accent never paints a requirement figure"
        );
    }

    /// Data figures keep their color: pie slices take the series, not
    /// the structural grays.
    #[test]
    fn pie_slices_paint_the_series() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let out = render("pie title Tasks\n    \"Done\" : 70\n    \"Todo\" : 30", &presentation)
            .expect("the pie renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        for index in 0..2 {
            assert!(
                svg.contains(&hex_string(presentation.palette.series[index])),
                "pie slice {index} paints its series color"
            );
        }
    }

    /// GitGraph branches are categories: the branch color is a series
    /// slot and its label the readable side of that slot.
    #[test]
    fn gitgraph_branches_paint_the_series() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let source = "gitGraph\n    commit id: \"INIT\"\n    branch develop\n    commit id: \"A\"\n    checkout main\n    commit id: \"B\"";
        let out = render(source, &presentation).expect("the gitgraph renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        assert!(
            svg.contains(&hex_string(presentation.palette.series[0])),
            "the first branch paints its series color"
        );
        assert!(
            svg.contains(&hex_string(presentation.palette.on_series[0])),
            "the branch label paints the readable side"
        );
    }

    /// Gantt's semantics survive the structural palette: critical
    /// tasks keep the danger border while their ground is repaired
    /// toward readable.
    #[test]
    fn gantt_critical_tasks_keep_the_danger_border() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let source = "gantt\n    dateFormat YYYY-MM-DD\n    section Section\n    A task :a1, 2024-01-01, 30d\n    Critical task :crit, 2024-01-01, 10d";
        let out = render(source, &presentation).expect("the gantt renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        let palette = &presentation.palette;
        assert!(
            svg.contains(&hex_string(palette.danger)),
            "the critical border keeps the danger color"
        );
        assert!(
            svg.contains(&hex_string(palette.status_surface(palette.danger))),
            "the critical ground is the repaired danger surface"
        );
    }

    /// A user's explicit node style is their intent: the adapter must
    /// not repaint a node the user colored.
    #[test]
    fn a_users_explicit_node_style_survives() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let out = render(
            "flowchart LR\n    A --> B\n    style A fill:#00ff00",
            &presentation,
        )
        .expect("the styled flowchart renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        assert!(
            svg.contains("#00ff00") || svg.contains("#00FF00"),
            "the user's green paints"
        );
    }

    /// A diagram's frontmatter config outranks the host defaults, the
    /// same precedence Mermaid itself runs: user, then host, then
    /// renderer. The variable is `mainBkg` — the one the flowchart
    /// fill actually reads; `primaryColor` alone does not derive into
    /// it under Merman 0.7's theme expansion.
    #[test]
    fn frontmatter_theme_variables_outrank_the_host() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let source =
            "%%{init: {\"themeVariables\": {\"mainBkg\": \"#123456\"}}}%%\nflowchart LR\n    A --> B";
        let out = render(source, &presentation).expect("the frontmatter flowchart renders");
        let svg = std::str::from_utf8(&out.svg).unwrap();
        assert!(
            svg.contains("#123456"),
            "the user's node fill paints, not the host surface"
        );
    }
}

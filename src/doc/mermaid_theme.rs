//! The Mermaid theme adapter: Oryx's reading theme becomes a semantic
//! diagram palette. Structural colors (what boxes, labels and lines
//! draw with) come only from structural roles — surface, blocks, text —
//! while the accent, the syntax rainbow and the alerts feed emphasis
//! marks and the categorical series. The projection into Merman's theme
//! variables is a later stage in this module; the renderer itself never
//! sees an Oryx `Theme`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::style::theme::{contrast, hex_string, Rgba, Theme};

/// Which way the palette reads: a light canvas carries dark marks. The
/// renderer derives every unset role from this, so it must agree with
/// the ground, not with the theme's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidAppearance {
    Light,
    Dark,
}

/// The body-text floor (WCAG AA) every reading surface must meet, and
/// the interface-mark floor for borders, lines and series colors.
pub const TEXT_FLOOR: f32 = 4.5;
pub const MARK_FLOOR: f32 = 3.0;

/// The repair ladder: how far a failing color steps toward its repair
/// target each try. Small steps first keep the theme's own hue; the
/// algorithm never snaps straight to black or white.
const REPAIR_STEPS: [f32; 6] = [0.15, 0.30, 0.45, 0.60, 0.75, 0.90];

/// How much of `surface_alt` joins the canvas to make the ground edge
/// and relationship labels sit on.
const LABEL_SURFACE_MIX: f32 = 0.60;

/// How much of a status color tints the canvas before the status
/// surface is repaired down to readable.
const STATUS_TINT: f32 = 0.20;

/// The number of categorical colors data diagrams draw with: Pie uses
/// up to twelve slices, GitGraph eight branches, everything else takes
/// what it needs from the front.
pub const SERIES_SLOTS: usize = 12;

/// The diagram palette: every color a Mermaid figure paints with,
/// resolved, opaque and contrast-checked. `series` and `on_series`
/// pair every categorical fill with the text or canvas that reads on
/// it.
#[derive(Debug, Clone, PartialEq)]
pub struct MermaidSemanticPalette {
    pub appearance: MermaidAppearance,
    pub canvas: Rgba,
    pub text: Rgba,
    pub muted_text: Rgba,
    pub surface: Rgba,
    pub surface_alt: Rgba,
    pub surface_muted: Rgba,
    pub label_surface: Rgba,
    pub border: Rgba,
    pub line: Rgba,
    pub accent: Rgba,
    pub info: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub series: [Rgba; SERIES_SLOTS],
    pub on_series: [Rgba; SERIES_SLOTS],
}

impl MermaidSemanticPalette {
    /// The reading theme as a diagram palette. Structural roles map to
    /// structural colors through the repair ladder; the link, syntax
    /// and alert colors become accent, status and series — never a box
    /// fill.
    pub fn from_oryx(theme: &Theme) -> Self {
        let canvas = opaque(theme.surface.background, WHITE);
        let appearance = if canvas.is_light() {
            MermaidAppearance::Light
        } else {
            MermaidAppearance::Dark
        };

        // Body text: the body role or the surface foreground, whichever
        // reads better on the ground, repaired toward the far pole when
        // even the better one fails.
        let mut text = best_of(theme.text.body, theme.surface.foreground, canvas);
        if contrast(text, canvas) < TEXT_FLOOR {
            text = repair_mark(text, appearance.pole(), canvas, TEXT_FLOOR);
        }

        // Muted text (legends, secondary labels): the comment role,
        // pulled toward the body text until it reads as a mark.
        let muted_raw = opaque(theme.syntax.comment, canvas);
        let muted_text = repair_mark(muted_raw, text, canvas, MARK_FLOOR);

        // Structural surfaces, each pulled toward the canvas until the
        // body text reads on it: the code panel, the table header, the
        // alternate row band, and the label ground the edges and
        // relationship names sit on.
        let surface = repair_surface(
            opaque(theme.blocks.code_bg, canvas),
            text,
            canvas,
            TEXT_FLOOR,
        );
        let surface_alt = repair_surface(
            opaque(theme.blocks.table_header_bg, canvas),
            text,
            canvas,
            TEXT_FLOOR,
        );
        let surface_muted = repair_surface(
            opaque(theme.blocks.table_row_alt_bg, canvas),
            text,
            canvas,
            TEXT_FLOOR,
        );
        let label_surface = repair_surface(
            mix(canvas, surface_alt, LABEL_SURFACE_MIX),
            text,
            canvas,
            TEXT_FLOOR,
        );

        // Borders read against the surface they close; the code border
        // wins unless the table border separates better.
        let code_border = opaque(theme.blocks.code_border, canvas);
        let table_border = opaque(theme.blocks.table_border, canvas);
        let border_candidate = if contrast(table_border, surface) > contrast(code_border, surface)
        {
            table_border
        } else {
            code_border
        };
        let border = repair_mark(border_candidate, text, surface, MARK_FLOOR);

        // Lines read against the ground, stepped toward the foreground
        // rather than defaulting to the strongest mark.
        let line = repair_mark(
            opaque(theme.blocks.rule, canvas),
            opaque(theme.surface.foreground, canvas),
            canvas,
            MARK_FLOOR,
        );

        let accent = opaque(theme.text.link, canvas);
        let info = opaque(theme.alerts.note, canvas);
        let success = opaque(theme.alerts.tip, canvas);
        let warning = opaque(theme.alerts.warning, canvas);
        let danger = opaque(theme.alerts.caution, canvas);

        let (series, on_series) = derive_series(theme, canvas, text, appearance.pole());

        MermaidSemanticPalette {
            appearance,
            canvas,
            text,
            muted_text,
            surface,
            surface_alt,
            surface_muted,
            label_surface,
            border,
            line,
            accent,
            info,
            success,
            warning,
            danger,
            series,
            on_series,
        }
    }

    /// The palette's own health check: structural colors opaque, the
    // body text reading on every surface it draws over, borders and
    // lines visible, and a categorical series that is both colorful and
    // readable. Every bundled theme must pass; a failure is a bug in
    /// the adapter, not in the theme.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let opaque_roles = [
            ("canvas", self.canvas),
            ("text", self.text),
            ("muted_text", self.muted_text),
            ("surface", self.surface),
            ("surface_alt", self.surface_alt),
            ("surface_muted", self.surface_muted),
            ("label_surface", self.label_surface),
            ("border", self.border),
            ("line", self.line),
            ("accent", self.accent),
            ("info", self.info),
            ("success", self.success),
            ("warning", self.warning),
            ("danger", self.danger),
        ];
        for (name, color) in opaque_roles {
            if color.a != 255 {
                problems.push(format!("{name} is not opaque: {:?}", hex_debug(color)));
            }
        }
        for (name, ground) in [
            ("canvas", self.canvas),
            ("surface", self.surface),
            ("surface_alt", self.surface_alt),
            ("label_surface", self.label_surface),
        ] {
            let ratio = contrast(self.text, ground);
            if ratio < TEXT_FLOOR {
                problems.push(format!(
                    "text reads {ratio:.2} on {name}, wanted {TEXT_FLOOR}"
                ));
            }
        }
        let muted_ratio = contrast(self.muted_text, self.canvas);
        if muted_ratio < MARK_FLOOR {
            problems.push(format!(
                "muted_text reads {muted_ratio:.2} on canvas, wanted {MARK_FLOOR}"
            ));
        }
        let border_ratio = contrast(self.border, self.surface);
        if border_ratio < MARK_FLOOR {
            problems.push(format!(
                "border reads {border_ratio:.2} on surface, wanted {MARK_FLOOR}"
            ));
        }
        let line_ratio = contrast(self.line, self.canvas);
        if line_ratio < MARK_FLOOR {
            problems.push(format!(
                "line reads {line_ratio:.2} on canvas, wanted {MARK_FLOOR}"
            ));
        }
        for (index, color) in self.series.iter().enumerate() {
            if color.a != 255 {
                problems.push(format!("series[{index}] is not opaque"));
            }
            let ratio = contrast(*color, self.canvas);
            if ratio < MARK_FLOOR {
                problems.push(format!(
                    "series[{index}] reads {ratio:.2} on canvas, wanted {MARK_FLOOR}"
                ));
            }
            let on = self.on_series[index];
            if on != self.text && on != self.canvas {
                problems.push(format!(
                    "on_series[{index}] is neither text nor canvas"
                ));
            }
            let on_ratio = contrast(on, *color);
            if on_ratio < MARK_FLOOR {
                problems.push(format!(
                    "on_series[{index}] reads {on_ratio:.2} on its series color"
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// A readable low-saturation ground for a status color: the alert
    /// tinted over the canvas and stepped back until the body text
    /// reads. The pure status color stays for the border, where
    /// saturation carries the meaning.
    pub fn status_surface(&self, status: Rgba) -> Rgba {
        repair_surface(mix(self.canvas, status, STATUS_TINT), self.text, self.canvas, MARK_FLOOR)
    }
}

impl MermaidAppearance {
    /// The pole a failing mark repairs toward: dark themes lift toward
    /// white, light themes sink toward black.
    fn pole(self) -> Rgba {
        match self {
            MermaidAppearance::Light => BLACK,
            MermaidAppearance::Dark => WHITE,
        }
    }
}

const WHITE: Rgba = Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const BLACK: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

/// Composites a translucent color over a ground: structural colors
/// enter the renderer opaque, because an alpha channel handed to a
/// diagram family is a color some CSS paths parse and others drop.
fn opaque(color: Rgba, ground: Rgba) -> Rgba {
    if color.a == 255 {
        return color;
    }
    let alpha = f32::from(color.a) / 255.0;
    let channel = |fg: u8, bg: u8| {
        (f32::from(fg) * alpha + f32::from(bg) * (1.0 - alpha)).round() as u8
    };
    Rgba {
        r: channel(color.r, ground.r),
        g: channel(color.g, ground.g),
        b: channel(color.b, ground.b),
        a: 255,
    }
}

/// Straight mix: `t` is how much of `b` joins `a`.
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let channel = |a: u8, b: u8| (f32::from(a) * (1.0 - t) + f32::from(b) * t).round() as u8;
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: 255,
    }
}

/// Whichever of two colors reads better on the ground.
fn best_of(a: Rgba, b: Rgba, ground: Rgba) -> Rgba {
    if contrast(a, ground) >= contrast(b, ground) {
        a
    } else {
        b
    }
}

/// Pulls a background toward the canvas until the text reads on it:
/// each step keeps more of the theme's hue than a flip to the extremes
/// would, and a background that already reads passes untouched.
fn repair_surface(mut ground: Rgba, text: Rgba, canvas: Rgba, floor: f32) -> Rgba {
    for step in REPAIR_STEPS {
        if contrast(text, ground) >= floor {
            break;
        }
        ground = mix(ground, canvas, step);
    }
    ground
}

/// Steps a mark toward a repair target until it meets its floor
/// against a ground.
fn repair_mark(mut color: Rgba, toward: Rgba, ground: Rgba, floor: f32) -> Rgba {
    for step in REPAIR_STEPS {
        if contrast(color, ground) >= floor {
            break;
        }
        color = mix(color, toward, step);
    }
    color
}

/// Whether two colors sit too close to share a categorical palette:
/// Euclidean RGB distance under 60 reads as the same color at diagram
/// size.
fn near(a: Rgba, b: Rgba) -> bool {
    let (dr, dg, db) = (
        i32::from(a.r) - i32::from(b.r),
        i32::from(a.g) - i32::from(b.g),
        i32::from(a.b) - i32::from(b.b),
    );
    dr * dr + dg * dg + db * db < 60 * 60
}

/// The categorical series and the text that reads on each slot. The
/// candidates are the theme's own emphasis colors — link, syntax
/// rainbow, alerts, headings — deduplicated, contrast-checked against
/// the canvas, and padded with variants of what survives, mixed toward
/// the pole away from the canvas so padding never trades readability
/// for count.
fn derive_series(
    theme: &Theme,
    canvas: Rgba,
    text: Rgba,
    pole: Rgba,
) -> ([Rgba; SERIES_SLOTS], [Rgba; SERIES_SLOTS]) {
    let candidates = [
        theme.text.link,
        theme.syntax.keyword,
        theme.syntax.function,
        theme.syntax.string,
        theme.syntax.number,
        theme.syntax.type_,
        theme.alerts.note,
        theme.alerts.tip,
        theme.alerts.important,
        theme.alerts.warning,
        theme.alerts.caution,
        theme.headings.h2,
        theme.headings.h3,
    ];
    let mut picked: Vec<Rgba> = Vec::with_capacity(SERIES_SLOTS);
    for raw in candidates {
        let color = opaque(raw, canvas);
        if contrast(color, canvas) < MARK_FLOOR {
            continue;
        }
        if picked.iter().any(|existing| near(*existing, color)) {
            continue;
        }
        picked.push(color);
        if picked.len() == SERIES_SLOTS {
            break;
        }
    }
    // A theme can offer fewer distinct readable colors than the data
    // diagrams want; grow the palette with variants of the survivors.
    // Bright bases crowd the pole, so after the graded ladder toward
    // the pole comes a pass of mixes between bases — new hues, still
    // readable marks — and every candidate must clear its neighbors.
    let bases = picked.clone();
    for round in 1..=12 {
        if picked.len() == SERIES_SLOTS {
            break;
        }
        let step = (round as f32 * 0.10).min(0.95);
        for base in &bases {
            if picked.len() == SERIES_SLOTS {
                break;
            }
            let candidate = mix(*base, pole, step);
            if contrast(candidate, canvas) < MARK_FLOOR {
                continue;
            }
            if picked.iter().any(|existing| near(*existing, candidate)) {
                continue;
            }
            picked.push(candidate);
        }
    }
    for i in 0..bases.len() {
        if picked.len() == SERIES_SLOTS {
            break;
        }
        let j = (i + 1) % bases.len();
        if bases.len() < 2 {
            break;
        }
        let candidate = mix(bases[i], bases[j], 0.5);
        if contrast(candidate, canvas) < MARK_FLOOR {
            continue;
        }
        if picked.iter().any(|existing| near(*existing, candidate)) {
            continue;
        }
        picked.push(candidate);
    }
    // Pathological themes can resist even that; search for any
    // distinct readable mix before falling back to count over variety.
    while picked.len() < SERIES_SLOTS {
        let filler = picked[picked.len() % bases.len().max(1)];
        let mut step = 0.05f32;
        let mut found = false;
        while step <= 0.95 {
            let candidate = mix(filler, pole, step);
            if contrast(candidate, canvas) >= MARK_FLOOR
                && !picked.iter().any(|existing| near(*existing, candidate))
            {
                picked.push(candidate);
                found = true;
                break;
            }
            step += 0.05;
        }
        if !found {
            picked.push(mix(filler, pole, 0.5));
        }
    }
    picked.truncate(SERIES_SLOTS);

    let mut series = [WHITE; SERIES_SLOTS];
    let mut on_series = [WHITE; SERIES_SLOTS];
    for (index, color) in picked.into_iter().enumerate() {
        series[index] = color;
        on_series[index] = best_of(text, canvas, color);
    }
    (series, on_series)
}

/// Debug shorthand for validation messages.
fn hex_debug(color: Rgba) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

/// The Mermaid theme-variable names this adapter sets, in one place.
/// The projection inserts through these constants and nothing else, so
/// a typo is a compile error, not a silently ignored variable. The
/// consumers of each name live in `docs/MERMAN_THEME_COVERAGE.md`.
mod vars {
    // common
    pub const BACKGROUND: &str = "background";
    pub const TEXT_COLOR: &str = "textColor";
    pub const PRIMARY_COLOR: &str = "primaryColor";
    pub const PRIMARY_TEXT_COLOR: &str = "primaryTextColor";
    pub const PRIMARY_BORDER_COLOR: &str = "primaryBorderColor";
    pub const MAIN_BKG: &str = "mainBkg";
    pub const NODE_TEXT_COLOR: &str = "nodeTextColor";
    pub const NODE_BORDER: &str = "nodeBorder";
    pub const SECONDARY_COLOR: &str = "secondaryColor";
    pub const SECONDARY_TEXT_COLOR: &str = "secondaryTextColor";
    pub const SECONDARY_BORDER_COLOR: &str = "secondaryBorderColor";
    pub const TERTIARY_COLOR: &str = "tertiaryColor";
    pub const TERTIARY_TEXT_COLOR: &str = "tertiaryTextColor";
    pub const TERTIARY_BORDER_COLOR: &str = "tertiaryBorderColor";
    pub const LINE_COLOR: &str = "lineColor";
    pub const EDGE_LABEL_BACKGROUND: &str = "edgeLabelBackground";
    pub const TITLE_COLOR: &str = "titleColor";
    pub const NOTE_BKG_COLOR: &str = "noteBkgColor";
    pub const NOTE_TEXT_COLOR: &str = "noteTextColor";
    pub const NOTE_BORDER_COLOR: &str = "noteBorderColor";
    pub const CLUSTER_BKG: &str = "clusterBkg";
    pub const CLUSTER_BORDER: &str = "clusterBorder";
    // sequence
    pub const ACTOR_BKG: &str = "actorBkg";
    pub const ACTOR_BORDER: &str = "actorBorder";
    pub const ACTOR_TEXT_COLOR: &str = "actorTextColor";
    pub const ACTOR_LINE_COLOR: &str = "actorLineColor";
    pub const SIGNAL_COLOR: &str = "signalColor";
    pub const SIGNAL_TEXT_COLOR: &str = "signalTextColor";
    pub const LABEL_BOX_BKG_COLOR: &str = "labelBoxBkgColor";
    pub const LABEL_BOX_BORDER_COLOR: &str = "labelBoxBorderColor";
    pub const LABEL_TEXT_COLOR: &str = "labelTextColor";
    pub const LOOP_TEXT_COLOR: &str = "loopTextColor";
    pub const ACTIVATION_BKG_COLOR: &str = "activationBkgColor";
    pub const ACTIVATION_BORDER_COLOR: &str = "activationBorderColor";
    pub const SEQUENCE_NUMBER_COLOR: &str = "sequenceNumberColor";
    // requirement
    pub const REQUIREMENT_BACKGROUND: &str = "requirementBackground";
    pub const REQUIREMENT_BORDER_COLOR: &str = "requirementBorderColor";
    pub const REQUIREMENT_TEXT_COLOR: &str = "requirementTextColor";
    pub const RELATION_COLOR: &str = "relationColor";
    pub const RELATION_LABEL_BACKGROUND: &str = "relationLabelBackground";
    pub const RELATION_LABEL_COLOR: &str = "relationLabelColor";
    pub const REQUIREMENT_EDGE_LABEL_BACKGROUND: &str = "requirementEdgeLabelBackground";
    // gantt
    pub const SECTION_BKG_COLOR: &str = "sectionBkgColor";
    pub const SECTION_BKG_COLOR2: &str = "sectionBkgColor2";
    pub const ALT_SECTION_BKG_COLOR: &str = "altSectionBkgColor";
    pub const GRID_COLOR: &str = "gridColor";
    pub const TODAY_LINE_COLOR: &str = "todayLineColor";
    pub const TASK_BKG_COLOR: &str = "taskBkgColor";
    pub const TASK_BORDER_COLOR: &str = "taskBorderColor";
    pub const TASK_TEXT_COLOR: &str = "taskTextColor";
    pub const TASK_TEXT_OUTSIDE_COLOR: &str = "taskTextOutsideColor";
    pub const TASK_TEXT_DARK_COLOR: &str = "taskTextDarkColor";
    pub const TASK_TEXT_CLICKABLE_COLOR: &str = "taskTextClickableColor";
    pub const ACTIVE_TASK_BKG_COLOR: &str = "activeTaskBkgColor";
    pub const ACTIVE_TASK_BORDER_COLOR: &str = "activeTaskBorderColor";
    pub const DONE_TASK_BKG_COLOR: &str = "doneTaskBkgColor";
    pub const DONE_TASK_BORDER_COLOR: &str = "doneTaskBorderColor";
    pub const CRIT_BKG_COLOR: &str = "critBkgColor";
    pub const CRIT_BORDER_COLOR: &str = "critBorderColor";
    pub const EXCLUDE_BKG_COLOR: &str = "excludeBkgColor";
    pub const VERT_LINE_COLOR: &str = "vertLineColor";
    // gitgraph
    pub const COMMIT_LABEL_COLOR: &str = "commitLabelColor";
    pub const COMMIT_LABEL_BACKGROUND: &str = "commitLabelBackground";
    pub const COMMIT_LINE_COLOR: &str = "commitLineColor";
    pub const TAG_LABEL_COLOR: &str = "tagLabelColor";
    pub const TAG_LABEL_BACKGROUND: &str = "tagLabelBackground";
    pub const TAG_LABEL_BORDER: &str = "tagLabelBorder";
    // pie
    pub const PIE_STROKE_COLOR: &str = "pieStrokeColor";
    pub const PIE_OUTER_STROKE_COLOR: &str = "pieOuterStrokeColor";
    pub const PIE_TITLE_TEXT_COLOR: &str = "pieTitleTextColor";
    pub const PIE_LEGEND_TEXT_COLOR: &str = "pieLegendTextColor";
    pub const PIE_SECTION_TEXT_COLOR: &str = "pieSectionTextColor";
    // architecture
    pub const ARCH_EDGE_COLOR: &str = "archEdgeColor";
    pub const ARCH_EDGE_ARROW_COLOR: &str = "archEdgeArrowColor";
    pub const ARCH_GROUP_BORDER_COLOR: &str = "archGroupBorderColor";
    // quadrant
    pub const QUADRANT_POINT_FILL: &str = "quadrantPointFill";
}

/// The palette flattened into Mermaid's own knobs: every common theme
/// variable the renderer reads, set to a semantic color. The map is
/// sorted, so the projection is byte-stable for a given palette —
/// which is what the cache keys on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MermaidThemeVariables {
    values: BTreeMap<String, String>,
}

impl MermaidThemeVariables {
    /// The common projection, design section "Common ThemeVariables":
    /// the ground, the text, the three surfaces, and the two line-ish
    /// roles. The load-bearing lines are `secondaryColor` and
    /// `tertiaryColor` — surface alternates and label grounds, never
    /// the accent.
    pub fn from_palette(palette: &MermaidSemanticPalette) -> Self {
        let mut vars = MermaidThemeVariables::default();
        vars.set(vars::BACKGROUND, palette.canvas);
        vars.set(vars::TEXT_COLOR, palette.text);
        vars.set(vars::PRIMARY_COLOR, palette.surface);
        vars.set(vars::MAIN_BKG, palette.surface);
        vars.set(vars::PRIMARY_TEXT_COLOR, palette.text);
        vars.set(vars::NODE_TEXT_COLOR, palette.text);
        vars.set(vars::PRIMARY_BORDER_COLOR, palette.border);
        vars.set(vars::NODE_BORDER, palette.border);
        vars.set(vars::SECONDARY_COLOR, palette.surface_alt);
        vars.set(vars::SECONDARY_TEXT_COLOR, palette.text);
        vars.set(vars::SECONDARY_BORDER_COLOR, palette.border);
        vars.set(vars::TERTIARY_COLOR, palette.label_surface);
        vars.set(vars::TERTIARY_TEXT_COLOR, palette.text);
        vars.set(vars::TERTIARY_BORDER_COLOR, palette.border);
        vars.set(vars::LINE_COLOR, palette.line);
        vars.set(vars::EDGE_LABEL_BACKGROUND, palette.label_surface);
        vars.set(vars::TITLE_COLOR, palette.text);
        vars.set(vars::NOTE_BKG_COLOR, palette.surface_alt);
        vars.set(vars::NOTE_TEXT_COLOR, palette.text);
        vars.set(vars::NOTE_BORDER_COLOR, palette.border);
        vars.set(vars::CLUSTER_BKG, palette.surface_muted);
        vars.set(vars::CLUSTER_BORDER, palette.border);
        vars.family(palette);
        vars
    }

    /// The family projection: the variables one diagram family reads
    /// that the common names do not cover. Every mapping follows
    /// `docs/MERMAN_THEME_COVERAGE.md` — structural families stay on
    /// the structural surfaces, data families take the series, and the
    /// status colors carry Gantt's semantics.
    fn family(&mut self, palette: &MermaidSemanticPalette) {
        self.sequence(palette);
        self.requirement(palette);
        self.gantt(palette);
        self.gitgraph(palette);
        self.pie(palette);
        self.architecture(palette);
        self.quadrant(palette);
        self.categorical(palette);
    }

    /// Sequence: actors are boxes — the surface — lifelines and
    /// signals are lines, control labels sit on the label ground, and
    /// activations are the alternate surface. None of it is accent.
    fn sequence(&mut self, palette: &MermaidSemanticPalette) {
        self.set(vars::ACTOR_BKG, palette.surface);
        self.set(vars::ACTOR_BORDER, palette.border);
        self.set(vars::ACTOR_TEXT_COLOR, palette.text);
        self.set(vars::ACTOR_LINE_COLOR, palette.line);
        self.set(vars::SIGNAL_COLOR, palette.line);
        self.set(vars::SIGNAL_TEXT_COLOR, palette.text);
        self.set(vars::LABEL_BOX_BKG_COLOR, palette.label_surface);
        self.set(vars::LABEL_BOX_BORDER_COLOR, palette.border);
        self.set(vars::LABEL_TEXT_COLOR, palette.text);
        self.set(vars::LOOP_TEXT_COLOR, palette.text);
        self.set(vars::ACTIVATION_BKG_COLOR, palette.surface_alt);
        self.set(vars::ACTIVATION_BORDER_COLOR, palette.border);
        self.set(vars::SEQUENCE_NUMBER_COLOR, palette.text);
    }

    /// Requirement: boxes on the surface, relations as lines, relation
    /// labels on the label ground — the same contract that fixed ER's
    /// green `contains`.
    fn requirement(&mut self, palette: &MermaidSemanticPalette) {
        self.set(vars::REQUIREMENT_BACKGROUND, palette.surface);
        self.set(vars::REQUIREMENT_BORDER_COLOR, palette.border);
        self.set(vars::REQUIREMENT_TEXT_COLOR, palette.text);
        self.set(vars::RELATION_COLOR, palette.line);
        self.set(vars::RELATION_LABEL_BACKGROUND, palette.label_surface);
        self.set(vars::RELATION_LABEL_COLOR, palette.text);
        self.set(vars::REQUIREMENT_EDGE_LABEL_BACKGROUND, palette.label_surface);
    }

    /// Gantt: structural lanes and grid, categorical task bars with
    /// their own readable text, done tasks as alternate surface, and
    /// critical tasks as a repaired danger ground with the pure alert
    /// on the border.
    fn gantt(&mut self, palette: &MermaidSemanticPalette) {
        self.set(vars::SECTION_BKG_COLOR, palette.surface_muted);
        self.set(vars::SECTION_BKG_COLOR2, palette.surface_alt);
        self.set(vars::ALT_SECTION_BKG_COLOR, palette.surface_muted);
        self.set(vars::GRID_COLOR, palette.border);
        self.set(vars::TODAY_LINE_COLOR, palette.accent);
        self.set(vars::TASK_BKG_COLOR, palette.series[0]);
        self.set(vars::TASK_BORDER_COLOR, palette.border);
        self.set(vars::TASK_TEXT_COLOR, palette.on_series[0]);
        self.set(vars::TASK_TEXT_OUTSIDE_COLOR, palette.text);
        self.set(vars::TASK_TEXT_DARK_COLOR, palette.text);
        self.set(vars::TASK_TEXT_CLICKABLE_COLOR, palette.accent);
        self.set(vars::ACTIVE_TASK_BKG_COLOR, palette.series[1]);
        self.set(vars::ACTIVE_TASK_BORDER_COLOR, palette.accent);
        self.set(vars::DONE_TASK_BKG_COLOR, palette.surface_alt);
        self.set(vars::DONE_TASK_BORDER_COLOR, palette.border);
        self.set(vars::CRIT_BKG_COLOR, palette.status_surface(palette.danger));
        self.set(vars::CRIT_BORDER_COLOR, palette.danger);
        self.set(vars::EXCLUDE_BKG_COLOR, palette.surface_alt);
        self.set(vars::VERT_LINE_COLOR, palette.warning);
    }

    /// GitGraph: branches are categories — the series — with the
    /// readable side of each color for their labels; commits and tags
    /// are structural.
    fn gitgraph(&mut self, palette: &MermaidSemanticPalette) {
        for index in 0..SERIES_SLOTS {
            self.set(&format!("git{index}"), palette.series[index]);
            self.set(&format!("gitInv{index}"), palette.on_series[index]);
            self.set(&format!("gitBranchLabel{index}"), palette.on_series[index]);
        }
        self.set(vars::COMMIT_LABEL_COLOR, palette.text);
        self.set(vars::COMMIT_LABEL_BACKGROUND, palette.label_surface);
        self.set(vars::COMMIT_LINE_COLOR, palette.line);
        self.set(vars::TAG_LABEL_COLOR, palette.text);
        self.set(vars::TAG_LABEL_BACKGROUND, palette.surface_alt);
        self.set(vars::TAG_LABEL_BORDER, palette.border);
    }

    /// Pie: slices are pure data — the series — with the canvas
    /// separating them and structural text around them.
    fn pie(&mut self, palette: &MermaidSemanticPalette) {
        for index in 0..SERIES_SLOTS {
            self.set(&format!("pie{}", index + 1), palette.series[index]);
        }
        self.set(vars::PIE_STROKE_COLOR, palette.canvas);
        self.set(vars::PIE_OUTER_STROKE_COLOR, palette.border);
        self.set(vars::PIE_TITLE_TEXT_COLOR, palette.text);
        self.set(vars::PIE_LEGEND_TEXT_COLOR, palette.text);
        self.set(vars::PIE_SECTION_TEXT_COLOR, palette.text);
    }

    /// Architecture: edges are lines, groups close with the border —
    /// structure, not accents.
    fn architecture(&mut self, palette: &MermaidSemanticPalette) {
        self.set(vars::ARCH_EDGE_COLOR, palette.line);
        self.set(vars::ARCH_EDGE_ARROW_COLOR, palette.line);
        self.set(vars::ARCH_GROUP_BORDER_COLOR, palette.border);
    }

    /// Quadrant: the chart is structural; the points are categorical
    /// marks and take the first series color.
    fn quadrant(&mut self, palette: &MermaidSemanticPalette) {
        self.set(vars::QUADRANT_POINT_FILL, palette.series[0]);
    }

    /// The indexed categorical scale the timeline, kanban, mindmap,
    /// treemap, venn and journey renderers read: every slot is a
    /// series color with its readable side alongside.
    fn categorical(&mut self, palette: &MermaidSemanticPalette) {
        for index in 0..SERIES_SLOTS {
            self.set(&format!("cScale{index}"), palette.series[index]);
            self.set(&format!("cScalePeer{index}"), palette.series[index]);
            self.set(&format!("cScaleLabel{index}"), palette.on_series[index]);
            self.set(&format!("cScaleInv{index}"), palette.on_series[index]);
            self.set(&format!("venn{}", index + 1), palette.series[index]);
            self.set(&format!("fillType{index}"), palette.series[index]);
            self.set(&format!("actor{index}"), palette.series[index]);
        }
    }

    fn set(&mut self, key: &str, color: Rgba) {
        self.values.insert(key.to_string(), hex_string(color));
    }

    /// One projected variable, for the contract tests.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// The variables as Merman takes them: a JSON object under the
    /// site config's `themeVariables`, merged over the host roles'
    /// derivations so the explicit contract wins.
    pub fn to_json(&self) -> Map<String, Value> {
        self.values
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect()
    }

    /// How many variables the projection sets, for the contract tests.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the projection set nothing at all.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// What the renderer receives: the palette, its projection into
/// Mermaid's theme variables, and the family-level diagram config that
/// does not ride through `themeVariables`. The adapter boundary —
/// nothing past this type knows an Oryx `Theme`.
#[derive(Debug, Clone, PartialEq)]
pub struct MermaidPresentation {
    pub palette: MermaidSemanticPalette,
    pub theme_variables: MermaidThemeVariables,
    pub family_config: Map<String, Value>,
}

impl MermaidPresentation {
    /// The reading theme as a full presentation.
    pub fn from_oryx(theme: &Theme) -> Self {
        let palette = MermaidSemanticPalette::from_oryx(theme);
        let theme_variables = MermaidThemeVariables::from_palette(&palette);
        MermaidPresentation {
            palette,
            theme_variables,
            family_config: Map::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::theme::parse_hex;

    fn hex(s: &str) -> Rgba {
        parse_hex(s).unwrap_or_else(|| panic!("bad hex {s}"))
    }

    /// The compiled-in dark theme projects a valid palette.
    #[test]
    fn the_dark_theme_projects_a_valid_palette() {
        let palette = MermaidSemanticPalette::from_oryx(&Theme::default_dark());
        assert_eq!(palette.appearance, MermaidAppearance::Dark);
        palette.validate().expect("the dark palette passes");
    }

    /// A light theme reads light, and its palette validates too.
    #[test]
    fn a_light_theme_reads_light() {
        let mut theme = Theme::default_dark();
        theme.surface.background = hex("#FFFFFF");
        theme.text.body = hex("#18181D");
        theme.surface.foreground = hex("#18181D");
        theme.blocks.code_bg = hex("#F3F4F8");
        let palette = MermaidSemanticPalette::from_oryx(&theme);
        assert_eq!(palette.appearance, MermaidAppearance::Light);
        palette.validate().expect("the light palette passes");
    }

    /// The neon regression theme: every accent role is eye-searing
    /// green, the structural roles stay neutral. The greens may paint
    /// accents and series, never a structural surface.
    fn neon_theme(dark: bool) -> Theme {
        let mut theme = Theme::default_dark();
        let (ground, body, code, header, alt, border_color) = if dark {
            (
                "#101010", "#E6E6E6", "#181818", "#222222", "#1C1C1C", "#3A3A3A",
            )
        } else {
            (
                "#FFFDF4", "#26221A", "#F5EFE0", "#EDE4CE", "#F8F4E8", "#C9BFa5",
            )
        };
        theme.surface.background = hex(ground);
        theme.surface.foreground = hex(body);
        theme.text.body = hex(body);
        theme.blocks.code_bg = hex(code);
        theme.blocks.table_header_bg = hex(header);
        theme.blocks.table_row_alt_bg = hex(alt);
        theme.blocks.code_border = hex(border_color);
        theme.blocks.table_border = hex(border_color);
        theme.blocks.rule = hex(border_color);
        theme.text.link = hex("#00FF00");
        theme.syntax.function = hex("#00FF00");
        theme.syntax.keyword = hex("#00FF00");
        theme.alerts.tip = hex("#00FF00");
        theme
    }

    #[test]
    fn neon_accents_never_become_structural_surfaces() {
        for dark in [false, true] {
            let palette = MermaidSemanticPalette::from_oryx(&neon_theme(dark));
            palette
                .validate()
                .unwrap_or_else(|err| panic!("the neon palette validates: {err}"));
            let green = hex("#00FF00");
            for (name, ground) in [
                ("surface", palette.surface),
                ("surface_alt", palette.surface_alt),
                ("surface_muted", palette.surface_muted),
                ("label_surface", palette.label_surface),
            ] {
                assert!(
                    !near(ground, green),
                    "neon green must not paint {name} (dark={dark})"
                );
            }
            assert_eq!(palette.accent, green, "the accent stays green");
            // Pure green cannot read on a light ground — the series
            // drops it by design — so only the dark ground, where the
            // neon accent itself reads, must keep a green-hued slot.
            if dark {
                let green_hued = |color: Rgba| {
                    i16::from(color.g) > i16::from(color.r) + 60
                        && i16::from(color.g) > i16::from(color.b) + 60
                };
                assert!(
                    palette.series.iter().any(|color| green_hued(*color)),
                    "the series keeps a green-hued slot"
                );
            }
        }
    }

    /// A broken-contrast theme — text and code panel nearly the same
    /// color — repairs through the ladder rather than flipping to pure
    /// black or white.
    #[test]
    fn broken_contrast_repairs_through_the_ladder() {
        let mut theme = Theme::default_dark();
        theme.blocks.code_bg = hex("#F8F8F2"); // body text, on a light panel
        let palette = MermaidSemanticPalette::from_oryx(&theme);
        palette
            .validate()
            .expect("repair makes the broken theme readable");
        assert!(
            palette.surface != hex("#FFFFFF") && palette.surface != hex("#000000"),
            "repair keeps the theme's hue, not the extremes"
        );
    }

    /// Translucent roles composite over the canvas before they reach
    /// the renderer: no alpha channel survives into a structural color.
    #[test]
    fn translucent_roles_composite_opaque() {
        let mut theme = Theme::default_dark();
        theme.blocks.code_bg = Rgba {
            r: 0x50,
            g: 0xFA,
            b: 0x7B,
            a: 0x80,
        };
        let palette = MermaidSemanticPalette::from_oryx(&theme);
        assert_eq!(palette.surface.a, 255, "the surface is opaque");
        assert_ne!(palette.surface, theme.blocks.code_bg);
        assert!(
            near(palette.surface, mix(theme.surface.background, theme.blocks.code_bg, 0x80 as f32 / 255.0)),
            "the surface composites over the canvas"
        );
    }

    /// The series holds twelve separable colors and every slot names
    /// the side that reads on it.
    #[test]
    fn the_series_is_full_and_polarized() {
        let palette = MermaidSemanticPalette::from_oryx(&Theme::default_dark());
        for index in 1..SERIES_SLOTS {
            assert!(
                !near(palette.series[index - 1], palette.series[index]),
                "neighbor slots separate"
            );
        }
        for (color, on) in palette.series.iter().zip(palette.on_series) {
            assert!(on == palette.text || on == palette.canvas);
            assert!(
                contrast(on, *color) >= MARK_FLOOR,
                "every slot reads {:?} on {:?}",
                hex_debug(on),
                hex_debug(*color)
            );
        }
    }

    /// Dracula's repeated roles collapse: the same color wearing three
    /// hats fills one slot, and the palette pads onward from there.
    #[test]
    fn duplicate_candidates_collapse_into_one_slot() {
        let theme = Theme::default_dark();
        let palette = MermaidSemanticPalette::from_oryx(&theme);
        for (i, a) in palette.series.iter().enumerate() {
            for b in palette.series.iter().skip(i + 1) {
                assert!(!near(*a, *b), "series slots are pairwise distinct");
            }
        }
    }

    /// A status ground is the alert tinted toward readable, with the
    /// body text still legible on it.
    #[test]
    fn status_surfaces_stay_readable() {
        let palette = MermaidSemanticPalette::from_oryx(&Theme::default_dark());
        for status in [palette.info, palette.success, palette.warning, palette.danger] {
            let ground = palette.status_surface(status);
            assert!(contrast(palette.text, ground) >= MARK_FLOOR);
            assert!(!near(ground, status), "the tint reads as a ground, not the alert");
        }
    }

    /// The muted text is the comment role when it reads, repaired
    /// toward the body text when it does not.
    #[test]
    fn muted_text_comes_from_the_comment_role() {
        let theme = Theme::default_dark();
        let palette = MermaidSemanticPalette::from_oryx(&theme);
        assert_eq!(palette.muted_text, theme.syntax.comment);
    }

    /// Opaque is plain alpha compositing.
    #[test]
    fn opaque_composites_channels() {
        let over = opaque(
            Rgba {
                r: 0xFF,
                g: 0x00,
                b: 0x00,
                a: 0x80,
            },
            WHITE,
        );
        assert_eq!(over, hex("#FF7F7F"));
        assert_eq!(
            opaque(
                Rgba {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255
                },
                BLACK
            ),
            Rgba {
                r: 1,
                g: 2,
                b: 3,
                a: 255
            }
        );
    }

    /// Mixing is linear in both endpoints.
    #[test]
    fn mix_blends_linearly() {
        assert_eq!(mix(BLACK, WHITE, 0.0), BLACK);
        assert_eq!(mix(BLACK, WHITE, 1.0), WHITE);
        assert_eq!(mix(hex("#101010"), hex("#F0F0F0"), 0.5), hex("#808080"));
    }

    /// The core projection contract, the one the ER bug fixed: the
    /// three surface tiers land where the renderer reads them, and
    /// none of them is the accent.
    #[test]
    fn the_common_projection_pins_the_surfaces() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let palette = &presentation.palette;
        let vars = &presentation.theme_variables;
        for (key, color) in [
            (vars::BACKGROUND, palette.canvas),
            (vars::PRIMARY_COLOR, palette.surface),
            (vars::MAIN_BKG, palette.surface),
            (vars::SECONDARY_COLOR, palette.surface_alt),
            (vars::TERTIARY_COLOR, palette.label_surface),
            (vars::CLUSTER_BKG, palette.surface_muted),
            (vars::EDGE_LABEL_BACKGROUND, palette.label_surface),
            (vars::LINE_COLOR, palette.line),
            (vars::NODE_BORDER, palette.border),
            (vars::NOTE_BKG_COLOR, palette.surface_alt),
        ] {
            assert_eq!(
                vars.get(key),
                Some(hex_string(color).as_str()),
                "{key} projects its palette role"
            );
        }
        for key in [
            vars::TEXT_COLOR,
            vars::PRIMARY_TEXT_COLOR,
            vars::NODE_TEXT_COLOR,
            vars::TITLE_COLOR,
        ] {
            assert_eq!(vars.get(key), Some(hex_string(palette.text).as_str()));
        }
    }

    /// The tertiary is a structural label surface, never the accent —
    /// the permanent guard for the green `contains` bug.
    #[test]
    fn the_tertiary_is_never_the_accent() {
        for theme in [Theme::default_dark(), neon_theme(true), neon_theme(false)] {
            let presentation = MermaidPresentation::from_oryx(&theme);
            let vars = &presentation.theme_variables;
            assert_eq!(
                vars.get(vars::TERTIARY_COLOR),
                Some(hex_string(presentation.palette.label_surface).as_str())
            );
            assert_ne!(
                vars.get(vars::TERTIARY_COLOR),
                Some(hex_string(presentation.palette.accent).as_str()),
                "tertiaryColor must not carry the accent"
            );
        }
    }

    /// The projection is deterministic and sorted: the same theme
    /// projects the same byte-for-byte map.
    #[test]
    fn the_projection_is_deterministic() {
        let a = MermaidPresentation::from_oryx(&Theme::default_dark());
        let b = MermaidPresentation::from_oryx(&Theme::default_dark());
        assert_eq!(a, b);
        assert!(!a.theme_variables.is_empty());
        assert_eq!(a.theme_variables.len(), b.theme_variables.len());
    }

    /// The family contract, straight from the coverage matrix: the
    /// sequence actor, the ER/requirement label grounds, the gitgraph
    /// commit label, the gantt task bars, and the indexed categorical
    /// scale all land where their renderers read them.
    #[test]
    fn the_family_projection_pins_the_contracts() {
        let presentation = MermaidPresentation::from_oryx(&Theme::default_dark());
        let palette = &presentation.palette;
        let vars = &presentation.theme_variables;
        for (key, color) in [
            (vars::ACTOR_BKG, palette.surface),
            (vars::LABEL_BOX_BKG_COLOR, palette.label_surface),
            (vars::ACTIVATION_BKG_COLOR, palette.surface_alt),
            (vars::RELATION_LABEL_BACKGROUND, palette.label_surface),
            (vars::REQUIREMENT_BACKGROUND, palette.surface),
            (vars::REQUIREMENT_EDGE_LABEL_BACKGROUND, palette.label_surface),
            (vars::COMMIT_LABEL_BACKGROUND, palette.label_surface),
            (vars::TAG_LABEL_BACKGROUND, palette.surface_alt),
            (vars::TASK_BKG_COLOR, palette.series[0]),
            (vars::TASK_TEXT_COLOR, palette.on_series[0]),
            (vars::ACTIVE_TASK_BKG_COLOR, palette.series[1]),
            (vars::CRIT_BORDER_COLOR, palette.danger),
            (vars::PIE_STROKE_COLOR, palette.canvas),
            (vars::ARCH_EDGE_COLOR, palette.line),
            (vars::QUADRANT_POINT_FILL, palette.series[0]),
        ] {
            assert_eq!(
                vars.get(key),
                Some(hex_string(color).as_str()),
                "{key} projects its palette role"
            );
        }
        for index in 0..SERIES_SLOTS {
            assert_eq!(
                vars.get(&format!("git{index}")),
                Some(hex_string(palette.series[index]).as_str()),
                "git{index} takes its series color"
            );
            assert_eq!(
                vars.get(&format!("gitBranchLabel{index}")),
                Some(hex_string(palette.on_series[index]).as_str())
            );
            assert_eq!(
                vars.get(&format!("pie{}", index + 1)),
                Some(hex_string(palette.series[index]).as_str())
            );
            assert_eq!(
                vars.get(&format!("cScaleLabel{index}")),
                Some(hex_string(palette.on_series[index]).as_str())
            );
        }
        // The critical-task ground is the danger repaired toward
        // readable, never the raw alert.
        let crit_ground = palette.status_surface(palette.danger);
        assert_eq!(
            vars.get(vars::CRIT_BKG_COLOR),
            Some(hex_string(crit_ground).as_str())
        );
        assert_ne!(crit_ground, palette.danger);
    }
}

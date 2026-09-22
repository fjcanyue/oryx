//! The all-theme quality gate for the Mermaid theme adapter: every
//! bundled theme must project a valid semantic palette, pin the core
//! theme-variable contract, and render plus rasterize the six core
//! diagram families. A neon regression theme guards the structural /
//! accent boundary from both sides — structure never catches the
//! neon, data figures never lose their color.

use std::collections::HashSet;
use std::path::PathBuf;

use oryx::doc::images::decode;
use oryx::doc::mermaid::{cache_key, render, MermaidPresentation};
use oryx::style::theme::{bundled_names, load_file, parse_hex, Theme};

fn themes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes")
}

fn bundled() -> Vec<(&'static str, Theme)> {
    bundled_names()
        .iter()
        .map(|name| {
            let theme =
                load_file(&themes_dir().join(format!("{name}.toml"))).unwrap_or_else(|| {
                    panic!("the bundled theme {name} loads")
                });
            (*name, theme)
        })
        .collect()
}

/// The six structural families the render matrix walks: they cover
/// node, actor, entity, class, state and requirement boxes — every
/// surface the accent must never paint.
const CORE: [(&str, &str); 6] = [
    (
        "flowchart",
        "flowchart LR\n    A[Start] --> B{Check}\n    B -->|ok| C[Done]",
    ),
    (
        "state",
        "stateDiagram-v2\n    [*] --> Idle\n    Idle --> Running\n    Running --> [*]",
    ),
    (
        "sequence",
        "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi",
    ),
    ("er", "erDiagram\n    TABLE_A ||--o{ TABLE_B : contains"),
    ("class", "classDiagram\n    Animal <|-- Duck"),
    (
        "requirement",
        "requirementDiagram\n    requirement req {\n        id: 1\n        text: the requirement\n        risk: high\n        verifymethod: analysis\n    }\n    element entity {\n        type: simulation\n    }\n    entity - satisfies -> req",
    ),
];

/// Layer one, no rendering: every bundled theme derives a palette that
/// holds its contrast floors, keeps its colors opaque, and fills a
/// readable categorical series.
#[test]
fn every_bundled_theme_projects_a_valid_palette() {
    let themes = bundled();
    assert!(themes.len() >= 30, "the full collection is present");
    let mut keys = HashSet::new();
    for (name, theme) in themes {
        let presentation = MermaidPresentation::from_oryx(&theme);
        presentation
            .palette
            .validate()
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        keys.insert(cache_key("flowchart LR\n    A --> B", &presentation));
    }
    assert_eq!(
        keys.len(),
        bundled_names().len(),
        "every theme keys its own cache entry"
    );
}

/// Layer two, no rendering: the projection contract holds under every
/// bundled theme — the three surface tiers, the sequence actor, the
/// label grounds, and the indexed series all land where the renderers
/// read them.
#[test]
fn every_bundled_theme_pins_the_core_projection() {
    use oryx::style::theme::hex_string;
    for (name, theme) in bundled() {
        let presentation = MermaidPresentation::from_oryx(&theme);
        let vars = &presentation.theme_variables;
        let palette = &presentation.palette;
        for (key, want) in [
            ("background", palette.canvas),
            ("primaryColor", palette.surface),
            ("mainBkg", palette.surface),
            ("secondaryColor", palette.surface_alt),
            ("tertiaryColor", palette.label_surface),
            ("clusterBkg", palette.surface_muted),
            ("edgeLabelBackground", palette.label_surface),
            ("actorBkg", palette.surface),
            ("labelBoxBkgColor", palette.label_surface),
            ("activationBkgColor", palette.surface_alt),
            ("relationLabelBackground", palette.label_surface),
            ("requirementBackground", palette.surface),
            ("requirementEdgeLabelBackground", palette.label_surface),
            ("commitLabelBackground", palette.label_surface),
            ("taskBkgColor", palette.series[0]),
            ("critBorderColor", palette.danger),
            ("git0", palette.series[0]),
            ("gitBranchLabel0", palette.on_series[0]),
            ("pie1", palette.series[0]),
            ("cScale0", palette.series[0]),
        ] {
            assert_eq!(
                vars.get(key),
                Some(hex_string(want).as_str()),
                "{name}: {key} projects its palette role"
            );
        }
        assert_ne!(
            vars.get("tertiaryColor"),
            vars.get("background"),
            "{name}: the label ground differs from the canvas"
        );
    }
}

/// Layer three, the render matrix: every bundled theme renders the six
/// core families and Oryx's own raster path accepts every svg.
#[test]
fn the_core_six_render_and_rasterize_under_every_bundled_theme() {
    for (name, theme) in bundled() {
        for (family, source) in CORE {
            let presentation = MermaidPresentation::from_oryx(&theme);
            let out = render(source, &presentation).unwrap_or_else(|err| {
                panic!("{name}: {family} renders: {err}")
            });
            assert!(
                out.width > 0.0 && out.height > 0.0,
                "{name}: {family} has a size"
            );
            decode(&out.svg)
                .unwrap_or_else(|| panic!("{name}: {family} rasterizes through Oryx"));
        }
    }
}

/// The neon regression theme, both appearances: structural roles
/// neutral, every accent role eye-searing green.
fn neon_theme(dark: bool) -> Theme {
    let mut theme = if dark {
        Theme::default_dark()
    } else {
        // A light base: the shipped light theme file.
        load_file(&themes_dir().join("oryx-light.toml")).expect("the light theme loads")
    };
    let green = parse_hex("#00FF00").expect("green parses");
    theme.text.link = green;
    theme.syntax.function = green;
    theme.syntax.keyword = green;
    theme.alerts.tip = green;
    theme
}

/// The boundary guard: under a neon theme no structural figure shows
/// the green, while the data figures keep their series colors — the
/// adapter neither leaks the accent into structure nor bleaches the
/// data charts.
#[test]
fn neon_accents_stay_out_of_structure_and_inside_the_data() {
    for dark in [false, true] {
        let presentation = MermaidPresentation::from_oryx(&neon_theme(dark));
        presentation
            .palette
            .validate()
            .unwrap_or_else(|err| panic!("the neon palette validates (dark={dark}): {err}"));

        // Structure: the six core families carry no pure green.
        for (family, source) in CORE {
            let out = render(source, &presentation)
                .unwrap_or_else(|err| panic!("{family} renders (dark={dark}): {err}"));
            let svg = std::str::from_utf8(&out.svg).unwrap();
            assert!(
                !svg.contains("#00FF00") && !svg.contains("#00ff00"),
                "{family} shows no neon green in its structure (dark={dark})"
            );
        }

        // Data: pie and gitgraph keep painting their series.
        let pie = render(
            "pie title Tasks\n    \"Done\" : 70\n    \"Todo\" : 30",
            &presentation,
        )
        .expect("the pie renders");
        let svg = std::str::from_utf8(&pie.svg).unwrap();
        assert!(
            svg.contains(&oryx::style::theme::hex_string(
                presentation.palette.series[0]
            )),
            "the neon pie still paints its first series color (dark={dark})"
        );
        assert_ne!(
            presentation.palette.series[0],
            presentation.palette.surface,
            "the series is not the structural surface (dark={dark})"
        );

        let git = render(
            "gitGraph\n    commit id: \"INIT\"\n    branch develop\n    commit id: \"A\"\n    checkout main\n    commit id: \"B\"",
            &presentation,
        )
        .expect("the gitgraph renders");
        let svg = std::str::from_utf8(&git.svg).unwrap();
        assert!(
            svg.contains(&oryx::style::theme::hex_string(
                presentation.palette.series[0]
            )),
            "the neon gitgraph still paints its first branch color (dark={dark})"
        );
    }
}

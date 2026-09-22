//! The real-world diagram sources the renderer is judged against,
//! beyond the hand-written one-liners in the unit tests. Fixtures live
//! in `tests/fixtures/mermaid/` (see its README); this file is the
//! migration's quality gate, in three layers: the structural bar every
//! svg clears, the raster bar Oryx's own decode path clears, and the
//! env-gated artifacts a human compares against the Mermaid Live
//! Editor.

use oryx::doc::images::decode;
use oryx::doc::mermaid::{cache_key, render, MermaidTheme};
use oryx::style::theme::Theme;

const FIXTURES: [&str; 10] = [
    "flowchart_basic.mmd",
    "flowchart_cjk_long.mmd",
    "state_basic.mmd",
    "state_cjk_business_flow.mmd",
    "state_back_edge.mmd",
    "sequence_cjk.mmd",
    "class_basic.mmd",
    "er_basic.mmd",
    "mindmap_cjk.mmd",
    "pie_basic.mmd",
];

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("mermaid")
            .join(name),
    )
    .unwrap_or_else(|err| panic!("the fixture {name} reads: {err}"))
}

/// The reading palettes the diagrams are judged under: the light
/// default and the compiled-in dark theme the app ships.
fn palettes() -> [(MermaidTheme, &'static str); 2] {
    [
        (MermaidTheme::default(), "light"),
        (MermaidTheme::from_oryx(&Theme::default_dark()), "dark"),
    ]
}

/// The structural bar, every fixture under both palettes: a finite
/// size, no NaN or Infinity in the geometry, and the resvg-safe
/// contract — no native `<foreignObject>` labels left for resvg to
/// drop on the floor.
#[test]
fn every_fixture_renders_a_structurally_sound_svg() {
    for name in FIXTURES {
        for (palette, label) in palettes() {
            let out = render(&fixture(name), &palette)
                .unwrap_or_else(|err| panic!("{name} renders under the {label} palette: {err}"));
            assert!(
                out.width > 0.0
                    && out.width.is_finite()
                    && out.height > 0.0
                    && out.height.is_finite(),
                "{name}/{label} answers a finite size: {}x{}",
                out.width,
                out.height
            );
            let svg = std::str::from_utf8(&out.svg).unwrap();
            assert!(
                !svg.contains("NaN") && !svg.contains("Infinity"),
                "{name}/{label} keeps its geometry finite"
            );
            assert_eq!(
                svg.matches("<foreignObject").count(),
                0,
                "{name}/{label} holds no native foreignObject labels"
            );
        }
    }
}

/// The raster bar: the svg Oryx will actually consume — the same
/// `decode` path the reading surface and the PDF export ride — turns
/// into pixels, not an error.
#[test]
fn every_fixture_rasterizes_through_oryx() {
    for name in FIXTURES {
        for (palette, label) in palettes() {
            let out = render(&fixture(name), &palette)
                .unwrap_or_else(|err| panic!("{name} renders under the {label} palette: {err}"));
            let pixels =
                decode(&out.svg).unwrap_or_else(|| panic!("{name}/{label} rasterizes in Oryx"));
            assert!(
                pixels.width() > 0 && pixels.height() > 0,
                "{name}/{label} answers pixels"
            );
        }
    }
}

/// With `ORYX_DUMP_MERMAID` set, every fixture under both palettes
/// lands its svg (the adapter's own dump) and a png rasterized through
/// Oryx's decode path, with an `index.txt` naming each hash. Unset —
/// every normal run — nothing is written: the product path stays pure
/// memory.
#[test]
fn fixtures_dump_for_visual_acceptance_when_asked() {
    if std::env::var_os("ORYX_DUMP_MERMAID").is_none() {
        return;
    }
    let dir = std::env::var_os("ORYX_DUMP_MERMAID_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target").join("mermaid-debug"));
    std::fs::create_dir_all(&dir).expect("the dump directory creates");
    let mut index = String::new();
    for name in FIXTURES {
        for (palette, label) in palettes() {
            let source = fixture(name);
            let out = render(&source, &palette)
                .unwrap_or_else(|err| panic!("{name} renders under the {label} palette: {err}"));
            let hash = cache_key(source.trim(), &palette)
                .uri()
                .trim_start_matches("mermaid://")
                .to_string();
            let svg = dir.join(format!("{hash}.svg"));
            assert!(svg.exists(), "the adapter wrote its svg: {}", svg.display());
            let pixels = decode(&out.svg).expect("Oryx rasterizes the diagram");
            pixels
                .save(dir.join(format!("{hash}.png")))
                .expect("the png writes");
            index.push_str(&format!("{hash} {name} {label}\n"));
        }
    }
    std::fs::write(dir.join("index.txt"), index).expect("the index writes");
}

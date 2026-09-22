//! The real-world diagram sources the renderer must stay good for,
//! beyond the hand-written one-liners in the unit tests. Fixtures live
//! in `tests/fixtures/mermaid/` (see its README); this file renders
//! them and, when `ORYX_DUMP_MERMAID` is set, checks the debug dump
//! and adds Oryx's own rasterization beside it — the artifacts a
//! renderer migration is judged against.

use oryx::doc::mermaid::{cache_key, render, MermaidTheme};

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

/// Renders without panic and answers a finite size. Layout quality —
/// overlap, label placement, back edges — is judged on the dumped
/// artifacts, not asserted here.
#[test]
fn the_business_state_flow_renders() {
    let out = render(
        &fixture("state_cjk_business_flow.mmd"),
        &MermaidTheme::default(),
    )
    .unwrap_or_else(|err| panic!("the business state flow renders: {err}"));
    assert!(out.width > 0.0 && out.width.is_finite());
    assert!(out.height > 0.0 && out.height.is_finite());
}

/// With `ORYX_DUMP_MERMAID` set the adapter writes its svg and this
/// test rasterizes the same bytes through Oryx's own decode path,
/// writing a png beside it. Unset — every normal run — the test only
/// proves the raster path accepts the svg and nothing is written.
#[test]
fn the_business_state_flow_dumps_and_rasterizes_when_asked() {
    let source = fixture("state_cjk_business_flow.mmd");
    let out = render(&source, &MermaidTheme::default()).unwrap();
    let pixels = oryx::doc::images::decode(&out.svg)
        .expect("Oryx's own raster path accepts the diagram svg");
    assert!(pixels.width() > 0 && pixels.height() > 0);
    if std::env::var_os("ORYX_DUMP_MERMAID").is_none() {
        return;
    }
    let hash = cache_key(source.trim(), &MermaidTheme::default())
        .uri()
        .trim_start_matches("mermaid://")
        .to_string();
    let dir = std::env::var_os("ORYX_DUMP_MERMAID_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target").join("mermaid-debug"));
    let svg = dir.join(format!("{hash}.svg"));
    assert!(svg.exists(), "the adapter wrote its svg: {}", svg.display());
    pixels
        .save(dir.join(format!("{hash}.png")))
        .expect("the png writes");
}

//! The dependency probe: Merman 0.7, strict parsing, the resvg-safe
//! pipeline, one trivial flowchart. It proves the toolchain and feature
//! set (`render` only) before any production code names the crate.

use merman::render::HeadlessRenderer;

#[test]
fn a_trivial_flowchart_renders_through_the_resvg_safe_pipeline() {
    let svg = HeadlessRenderer::new()
        .with_strict_parsing()
        .render_svg_resvg_safe_sync("flowchart TD\n    A --> B")
        .expect("the renderer succeeds")
        .expect("a flowchart is detected");
    assert!(svg.starts_with("<svg"), "an svg document, got: {svg:.60}");
    assert!(svg.contains("viewBox="), "the size rides the viewBox");
}

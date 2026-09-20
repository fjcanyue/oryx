//! Phase 0 smoke test: the mermaid dependency builds SVG-only (no CLI,
//! no PNG stack) and a trivial flowchart renders to an SVG string.
//! Migrates into the adapter module's own tests once one exists.

#[test]
fn a_trivial_flowchart_renders_to_svg() {
    let svg =
        mermaid_rs_renderer::render("flowchart LR\nA --> B").expect("a trivial flowchart renders");
    assert!(
        svg.trim_start().starts_with("<svg"),
        "the output is an svg document: {svg}"
    );
}

//! The dependency probe: Merman 0.8, strict parsing, the resvg-safe
//! pipeline, one trivial flowchart. It proves the toolchain and feature
//! set (`complete-svg` only) before any production code names the crate.

use merman::svg::{RootBackgroundPostprocessor, SvgPipeline, SvgRenderOptions};
use merman::{
    Engine, MermaidConfig, OperationControl, ParseOptions, RenderOutput, RenderRequest,
    Renderer, SvgRequest,
};

#[test]
fn a_trivial_flowchart_renders_through_the_resvg_safe_pipeline() {
    let renderer = Renderer::new()
        .with_engine(
            Engine::new()
                .with_site_config(MermaidConfig::from_value(serde_json::json!({}))),
        )
        .with_parse_options(ParseOptions::strict());
    let request = RenderRequest::svg(
        "flowchart TD\n    A --> B",
        OperationControl::new(),
        SvgRequest {
            options: SvgRenderOptions::default(),
            pipeline: Some(
                SvgPipeline::resvg_safe()
                    .with_postprocessor(RootBackgroundPostprocessor::new("#ffffff")),
            ),
            ..SvgRequest::default()
        },
    );
    let output = renderer.render(request).expect("the renderer succeeds");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("a flowchart is detected");
    };
    let (svg, _evidence) = svg.into_parts();
    assert!(svg.starts_with("<svg"), "an svg document, got: {svg:.60}");
    assert!(svg.contains("viewBox="), "the size rides the viewBox");
}

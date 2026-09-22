//! Renderer throughput numbers, printed, never asserted: timings vary
// with machine and load, so they gate nothing. Run explicitly with
//!     cargo test --release --test mermaid_perf -- --ignored --nocapture
//! and read the numbers into the migration report.

use std::time::Instant;

use oryx::doc::mermaid::{render, MermaidTheme};

fn business_flow() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("mermaid")
            .join("state_cjk_business_flow.mmd"),
    )
    .expect("the fixture reads")
}

/// Thirty nodes, every node also branching: routing work, not just a
/// chain.
fn large_flowchart() -> String {
    let mut source = String::from("flowchart LR\n");
    for i in 0..30 {
        source.push_str(&format!("    N{i} --> N{}\n", i + 1));
        source.push_str(&format!("    N{i} --> F{i}: 分支处理\n"));
    }
    source
}

/// A state chain long enough to push the layout, with a third of the
/// states branching through a detour state and back.
fn large_state() -> String {
    let mut source = String::from("stateDiagram-v2\n    [*] --> S0\n");
    for i in 0..30 {
        source.push_str(&format!("    S{i} --> S{}\n", i + 1));
        if i % 3 == 0 {
            source.push_str(&format!("    S{i} --> D{i}: 分支检测\n"));
            source.push_str(&format!("    D{i} --> S{}: 分支恢复\n", i + 1));
        }
    }
    source.push_str("    S30 --> [*]\n");
    source
}

/// Six participants, forty CJK messages.
fn large_sequence() -> String {
    let mut source = String::from("sequenceDiagram\n");
    for p in 0..6 {
        source.push_str(&format!("    participant P{p}\n"));
    }
    for i in 0..40 {
        source.push_str(&format!(
            "    P{}->>P{}: 消息调用编号{i}\n",
            i % 6,
            (i + 1) % 6
        ));
    }
    source
}

fn timed(label: &str, source: &str) {
    let start = Instant::now();
    let out = render(source, &MermaidTheme::default()).expect("renders");
    println!(
        "{label}: {:?} ({} x {})",
        start.elapsed(),
        out.width as u32,
        out.height as u32
    );
}

#[test]
#[ignore]
fn renderer_throughput() {
    timed("simple flowchart", "flowchart TD\n    A --> B");
    timed("real CJK state", &business_flow());
    timed("large state", &large_state());
    timed("large sequence", &large_sequence());
    timed("large flowchart", &large_flowchart());
    for count in [10, 50] {
        let source = business_flow();
        let start = Instant::now();
        for _ in 0..count {
            render(&source, &MermaidTheme::default()).expect("renders");
        }
        let total = start.elapsed();
        println!("{count} diagrams: {total:?} ({:?} each)", total / count);
    }
}

# Mermaid fixtures

Real diagram sources the renderer is judged against. A fixture is a
`.mmd` file holding exactly the fenced block content a document carries.

## Reproducing the artifacts

```
ORYX_DUMP_MERMAID=1 cargo test --test mermaid_fixtures
```

writes, per fixture, the adapter's own svg and the png Oryx's raster
path makes of it, under `target/mermaid-debug/` (override with
`ORYX_DUMP_MERMAID_DIR`). Without the variable nothing is written: the
product path stays pure memory. Compare the png against the Mermaid
Live Editor rendering of the same source — readable, nothing severely
overlapped — rather than pixel-for-pixel.

## state_cjk_business_flow.mmd

The business state flow (中文状态 + 长 transition label + 分支 + 回边)
whose rendering triggered the Merman migration. The labels
`排障后人工重跑（幂等重装）` and `Init()/InitialVar 任一步非0(71xx错误码)`
are verbatim from the original document; the states around them keep
the same shape: branches, back edges, long CJK labels, `[*]` ends.

### Baseline: mermaid-rs-renderer 0.3.1 — FAIL

Observed in the dumped artifacts:

- transition labels overlap each other
- nodes overlap labels
- CJK text placement incorrect
- back edge layout compressed

### Merman 0.7.0 — PASS (2026-09-22)

Judged on the dumped artifacts under both palettes against the
acceptance checklist (not pixel-perfect against the Mermaid Live
Editor):

- state nodes do not overlap each other, under light and dark
- transition labels do not cover states
- the three parallel edges around 正常运行/升级中 stay
  distinguishable; boxes may touch, reading does not break
- CJK text stays inside its label backgrounds, long labels wrap
- back edges (故障→未安装, 降级运行→正常运行) carry their own paths
- `[*]` start and end nodes present and correct
- the automated layers hold: finite sizes, no NaN/Infinity, no native
  `<foreignObject>`, every fixture rasterizes through Oryx's own
  `decode`

Remaining known imperfection, accepted: Merman spaces the labels of
parallel back-and-forth state edges tightly; per the migration design
no offset patching happens host-side. Fixtures judged 7–9/10 readable;
`state_cjk_business_flow` 7/10, every other fixture 9–10/10.

## Throughput (2026-09-22, Windows, release, one core of the dev box)

`cargo test --release --test mermaid_perf -- --ignored --nocapture`:

| case | latency | size |
| --- | --- | --- |
| simple flowchart | 4.4 ms | 86×174 |
| real CJK state (`state_cjk_business_flow`) | 28.7 ms | 969×714 |
| large state (40 states, third branching) | 168 ms | 105×4264 |
| large sequence (6 actors × 40 messages) | 3.9 ms | 1250×1931 |
| large flowchart (30 nodes + 30 branches) | 41 ms | 6117×1682 |
| 10 diagrams | 201 ms total | ~20 ms each |
| 50 diagrams | 1.05 s total | ~21 ms each |

Per-diagram cost stays flat as the count grows, so the per-render
`HeadlessRenderer` construction and host-theme compilation cost
nothing worth caching — the pool workers (1–4, unchanged) absorb the
load off the UI thread. Release binary: 33.0 MB with Merman against
22.6 MB before (+10.4 MB, the parity layout stack). Memory is not
instrumented; the MediaCache byte budget is unchanged.

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

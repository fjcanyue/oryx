# Merman 0.7 主题变量覆盖矩阵（Theme Coverage Inventory）

本文档是 Oryx ↔ Merman 的主题契约：固定 `merman = "=0.7.0"`（`Cargo.lock` pin）
下，按 Diagram Family 记录 renderer 实际消费的颜色变量、视觉语义、语义类别、
以及 Oryx 侧的语义角色来源。升级 Merman 后必须重新审计（重新生成本表），
因为 family renderer 消费的变量可能改变。

数据来源：`merman-render-0.7.0/src/svg/theme_profile.rs`（host roles →
themeVariables 的中心映射 `put_theme_roles` / `put_series_palette` /
`put_diagram_config`）、`src/svg/parity/theme.rs`（`PresentationTheme` 按
family 解析变量）、以及各 family 的 `css.rs` / renderer 源码。

## 注入链路与优先级

```text
HostThemeProfile.roles          → put_theme_roles   → 常规 themeVariables（长尾）
HostThemeProfile.series_palette → put_series_palette → 索引色（git/pie/cScale/venn/…）
HostThemeProfile.theme_variables ────────merge 最后──────► 覆盖以上派生值
HostThemeProfile.site_config    → 根级 diagram config（packet/treemap/radar/c4）
────────────────────────────────────────────────────────────
图源 frontmatter / %%{init}%%    → 在 site_config 之上合并（用户配置优先）
classDef / style / inline style  → 最高（源码级样式）
```

Oryx 的投影：`MermaidPresentation`（`src/doc/mermaid_theme.rs`）= palette +
显式 theme_variables（覆盖派生）+ family_config（site_config）。Oryx 显式
设置的变量全部写入 `theme_variables`，因此对 Merman 派生值保持确定胜出。

## 语义类别图例

- **Structural**：只能来自 canvas / surface* / label_surface / border / line / text。
- **Categorical**：数据系列区分，来自 series[]。
- **Status**：语义状态，来自 alerts（info/success/warning/danger 及其 status_surface）。
- **Accent**：高饱和强调，仅用于真正的强调标记。

## Common（所有 family 共享基础）

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `background` | 画布底色 | Structural | `canvas` |
| `textColor` | 基础文字 | Structural | `text` |
| `lineColor` | 连线/箭头基色 | Structural | `line` |
| `errorBkgColor` / `errorTextColor` | 错误占位 | Status | `danger` / `text` |
| `fontFamily` / `fontSize` | 字体 | — | body 字体 |

## 通用节点图（Flowchart / Class / State / Block / Ishikawa / Info）

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `primaryColor` / `mainBkg` | 节点填充 | Structural | `surface` |
| `nodeBorder` / `primaryBorderColor` | 节点描边 | Structural | `border` |
| `nodeTextColor` / `primaryTextColor` | 节点文字 | Structural | `text` |
| `titleColor` | 标题 | Structural | `text` |
| `tertiaryColor` | 次级容器/衍生底 | Structural | `label_surface` |
| `edgeLabelBackground` | 边标签底 | Structural | `label_surface` |
| `clusterBkg` / `clusterBorder` | 子图容器 | Structural | `surface_muted` / `border` |
| `arrowheadColor` | 箭头 | Structural | `line` |
| `stateBkg` / `stateBorder` / `stateLabelColor` | 状态节点 | Structural | `surface` / `border` / `text` |
| `transitionColor` / `transitionLabelColor` | 状态迁移 | Structural | `line` / `text` |
| `compositeBackground` / `compositeTitleBackground` | 组合状态底 | Structural | `canvas` |
| `altBackground` | 交替分区底 | Structural | `surface_muted` |
| `labelBackgroundColor` | 标签底 | Structural | `label_surface` |
| `rowOdd` / `rowEven` | 类图属性行交替底 | Structural | `surface` / `surface_alt` |
| `noteBkgColor` / `noteTextColor` / `noteBorderColor` | 注释框 | Structural | `surface_alt` / `text` / `border` |

## Sequence

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `actorBkg` | actor 盒填充 | Structural | `surface` |
| `actorBorder` | actor 盒描边 | Structural | `border` |
| `actorTextColor` | actor 文字 | Structural | `text` |
| `actorLineColor` | 生命线 | Structural | `line` |
| `signalColor` / `signalTextColor` | 消息线与文字 | Structural | `line` / `text` |
| `labelBoxBkgColor` | 控制块（loop/alt）标签底 | Structural | `label_surface` |
| `labelBoxBorderColor` | 控制块标签描边 | Structural | `border` |
| `labelTextColor` / `loopTextColor` | 控制块文字 | Structural | `text` |
| `activationBkgColor` / `activationBorderColor` | 激活条 | Structural | `surface_alt` / `border` |
| `sequenceNumberColor` | 消息序号 | Structural | `text` |
| `noteBkgColor` / `noteTextColor` / `noteBorderColor` | Note | Structural | `surface_alt` / `text` / `border` |

## ER

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `mainBkg` | `.entityBox` 填充 | Structural | `surface` |
| `nodeBorder` | `.entityBox` 描边 | Structural | `border` |
| `nodeTextColor` | 实体文字 | Structural | `text` |
| `lineColor` | `.relationshipLine` | Structural | `line` |
| `tertiaryColor` | `.relationshipLabelBox` 填充 | Structural | `label_surface` |
| `edgeLabelBackground` | 边标签底 | Structural | `label_surface` |
| `erEdgeLabelBackground` | ER 专属标签底 | — | **0.7 仅在 theme 名为 `redux-color`/`redux-dark-color` 时读取**；host 主题名为 `base`，不生效，勿设置 |

## Requirement

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `requirementBackground` | 需求盒填充 | Structural | `surface` |
| `requirementBorderColor` | 需求盒描边 | Structural | `border` |
| `requirementTextColor` | 需求文字 | Structural | `text` |
| `relationColor` | 关系线 | Structural | `line` |
| `relationLabelBackground` | 关系标签底 | Structural | `label_surface` |
| `relationLabelColor` | 关系标签文字 | Structural | `text` |
| `requirementEdgeLabelBackground` | 元素标签底 | Structural | `label_surface` |
| `edgeLabelBackground` | 边标签底 | Structural | `label_surface` |

## Gantt

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `sectionBkgColor` | 分区底 | Structural | `surface_muted` |
| `sectionBkgColor2` | 分区底（第二色） | Structural | `surface_alt` |
| `altSectionBkgColor` | 交替分区底 | Structural | `surface_muted` |
| `gridColor` | 网格线 | Structural | `border` |
| `todayLineColor` | 今日线 | Accent | `accent` |
| `taskBkgColor` | 任务条 | Categorical | `series[0]` |
| `taskBorderColor` | 任务条描边 | Structural | `border` |
| `taskTextColor` | 任务条内文字 | Categorical | `on_series[0]` |
| `taskTextOutsideColor` | 条外文字 | Structural | `text` |
| `taskTextDarkColor` | 深色任务文字 | Structural | `text` |
| `taskTextClickableColor` | 可点击任务文字 | Accent | `accent` |
| `activeTaskBkgColor` | 活动任务条 | Categorical | `series[1]` |
| `activeTaskBorderColor` | 活动任务描边 | Accent | `accent` |
| `doneTaskBkgColor` / `doneTaskBorderColor` | 完成任务 | Structural | `surface_alt` / `border` |
| `critBkgColor` | 关键任务底 | Status | `status_surface(danger)` |
| `critBorderColor` | 关键任务描边 | Status | `danger` |
| `excludeBkgColor` | 排除区间底 | Structural | `surface_alt` |
| `vertLineColor` | 竖直刻度线 | Status | `warning` |
| `titleColor` / `titleTextColor` | 标题 | Structural | `text` |

## GitGraph

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `git0`..`git7` | 分支提交点/线 | Categorical | `series[0..8]`（超过 8 分支 renderer 循环取模） |
| `gitInv0`..`git7` | 分支反色（填充对比） | Categorical | `on_series[0..8]` |
| `gitBranchLabel0`..`git7` | 分支标签文字 | Categorical | `on_series[0..8]` |
| `commitLabelColor` / `commitLabelBackground` | 提交标签 | Structural | `text` / `label_surface` |
| `commitLineColor` | 提交连线 | Structural | `line` |
| `tagLabelColor` / `tagLabelBackground` / `tagLabelBorder` | 标签（tag） | Structural | `text` / `surface_alt` / `border` |

## Pie

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `pie1`..`pie12` | 扇区（按 label 顺序循环） | Categorical | `series[0..12]` |
| `pieStrokeColor` | 扇区间隔描边 | Structural | `canvas` |
| `pieOuterStrokeColor` | 外圈描边 | Structural | `border` |
| `pieTitleTextColor` | 标题 | Structural | `text` |
| `pieSectionTextColor` | 扇区文字 | Structural | `text` |
| `pieLegendTextColor` | 图例文字 | Structural | `muted_text` |

## Categorical Scale（Timeline / Kanban / Mindmap / Treemap / Venn / Journey / XYChart / Radar）

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `cScale0`..`cScale11` | 分区/曲线/节点系列色 | Categorical | `series[N]` |
| `cScalePeer0`..`cScalePeer11` | 同系列伴随填充 | Categorical | `series[N]` |
| `cScaleLabel0`..`cScale11` | 系列标签文字 | Categorical | `on_series[N]` |
| `cScaleInv0`..`cScaleInv11` | 系列反色 | Categorical | `on_series[N]` |
| `venn1`..`venn12` | Venn 圆 | Categorical | `series[N]` |
| `vennTitleTextColor` / `vennSetTextColor` | Venn 文字 | Structural | `text` |
| `fillType0`..`fillType11` | Journey 面部/填充类型 | Categorical | `series[N]` |
| `actor0`..`actor11` | Journey 参与者 | Categorical | `series[N]` |
| `xyChart.plotColorPalette` | XY 曲线（逗号串） | Categorical | `series` joined |
| `xyChart.accentColor` | XY 强调色 | Categorical | `series[0]` |
| `xyChart.backgroundColor` | XY 画布 | Structural | `canvas` |
| `xyChart.titleColor` / 轴色 | XY 标题/轴 | Structural | `text` / `line` |
| `faceColor` | Journey 笑脸 | Structural | `surface` |
| `border2` | Journey 边框 | Structural | `border` |
| `tertiaryColor`（Timeline disabled_fill / Journey） | 禁用/衍生底 | Structural | `label_surface` |

## Architecture

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `archEdgeColor` / `archEdgeArrowColor` | 边与箭头 | Structural | `line` |
| `archGroupBorderColor` | 分组边框 | Structural | `border` |
| `textColor` / `lineColor` / `primaryBorderColor` | 基础 | Structural | `text` / `line` / `border` |

## Sankey

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `mainBkg` / `background` | 标签底 | Structural | `surface` / `canvas` |
| `textColor` | 标签文字 | Structural | `text` |
| `sankey.nodeColors`（diagram config） | 节点色映射 | Categorical | **按图配置，键为节点 id，主题层无法预知；0.7 默认内置 tableau10**。需要时由用户按图通过 frontmatter 覆盖 |

## Quadrant

| Variable | 视觉元素 | 类别 | Oryx 角色 |
| --- | --- | --- | --- |
| `quadrant1..4Fill` | 象限底 | Structural | `surface` / `surface_alt` / `canvas` / `surface_muted` |
| `quadrant1..4TextFill` | 象限文字 | Structural | `text` |
| `quadrantPointFill` | 数据点 | Categorical | `series[0]` |
| `quadrantPointTextFill` / `quadrantTitleFill` | 点/标题文字 | Structural | `text` |
| `quadrantInternal/ExternalBorderStrokeFill` | 象限分隔线 | Structural | `border` |

## 根级 diagram config（`site_config`，非 themeVariables）

Merman 0.7 的 `put_diagram_config` 从 host roles 自动派生以下配置，Oryx
设置语义 roles 后即被覆盖，无需在 family_config 重复：

| Config | 键 | 来源 role |
| --- | --- | --- |
| `packet` | `startByteColor`/`endByteColor`/`labelColor`/`titleColor`/`blockStrokeColor`/`blockFillColor` | line/border/text/surface |
| `treemap` | `titleColor`/`labelColor`/`valueColor`/`sectionStrokeColor`/`sectionFillColor`/`leafStrokeColor`/`leafFillColor` | text/line/border/surface_alt/surface；分区实际按 `cScaleN` 系列着色 |
| `radar` | `axisColor`/`graticuleColor` | line/border |
| `treeView`（themeVariables 嵌套） | `labelColor`/`lineColor` | text/line |
| `c4` | 20 组 `{prefix}_bg_color`/`{prefix}_border_color` | surface/border |
| eventmodeling（em*） | `emUiFill`…`emEventStroke` | surface/border/line + series[0..4] |
| `sankey` | `nodeColors`/`linkColor` 等 | 见上；主题层不设置 |

## 基线问题（本设计的直接动因，已由新投影修复）

| 现象 | 根因链 |
| --- | --- |
| Sequence actor 盒意外绿色 | 旧 `MermaidTheme::host_profile` 把 `surface_alt` 设为 `accent`（`text.link`）→ `put_theme_roles` 的 `actorBkg ← actor_background.or(surface_alt)` 吃到 accent |
| ER 关系标签 `contains` 意外绿色 | 同一 `surface_alt = accent` → `tertiaryColor ← surface_muted.or(surface_alt)` → `.relationshipLabelBox{fill:tertiaryColor}` |

修复后契约：`secondaryColor = surface_alt`、`tertiaryColor = label_surface`，
Accent 永不进入任何 structural fill。回归测试固定于
`src/doc/mermaid.rs` 与 `tests/mermaid_theme_matrix.rs`。

# Merman 0.8 主题变量覆盖矩阵（Theme Coverage Inventory）

本文档是 Oryx ↔ Merman 的主题契约：固定 `merman = "=0.8.0"`（`Cargo.lock` pin，
`all-diagrams + complete-svg`，无 ELK）下，按 Diagram Family 记录 renderer 实际
消费的颜色变量、视觉语义、语义类别、以及 Oryx 侧的语义角色来源。升级 Merman
后必须重新审计（重新生成本表），因为 family renderer 消费的变量可能改变。

数据来源：`merman-render-0.8.0/src/svg/parity/theme/families.rs`（各 family
按 `raw.color(<变量>, <默认>)` 直读 themeVariables）、`src/svg/parity/css.rs`
（theme 名相关的例外分支）、`src/presentation/`（HostTheme 编译层——Oryx 不经
过，仅作对照）、以及各 family 的 `css.rs` / renderer 源码。0.8 重审计
（2026-10-12）：变量消费面与 0.7 一致，selector 名（`.entityBox`、
`.relationshipLabelBox` 等）不变。

## 注入链路与优先级（0.8）

```text
Oryx site config（Engine::with_site_config）
  ├─ themeVariables = MermaidPresentation 投影 + fontFamily（全部显式）
  └─ family_config 键（根级 diagram config，现时为空）
────────────────────────────────────────────────────────────
图源 frontmatter / %%{init}%%    → 在 site config 之上合并（用户配置优先）
classDef / style / inline style  → 最高（源码级样式）
```

0.7 的 `HostThemeProfile.roles → put_theme_roles` 派生层在 Oryx 路径中不再
经过（Oryx 投影显式覆盖全部契约变量，含 git/pie/cScale/venn/fillType 全系
列 categorical）。0.8 的 `presentation/compiler.rs` 仍为 HostTheme 用户提供
等价编译，Oryx 不使用。

注意一个跨版本保持的契约细节（0.7 与 0.8 相同）：`%%{init}%%` 里只写
`primaryColor` 不会派生进 flowchart 的节点填充——family 解析是
`raw.color("mainBkg", <默认>)` 直读，不从 `primaryColor` 推导。用户要改节点
填充需直接写 `mainBkg`（Oryx 的投影两个都设，故 host 侧无此问题）。

Oryx 的投影：`MermaidPresentation`（`src/doc/mermaid_theme.rs`）= palette +
显式 theme_variables + family_config。Oryx 显式设置的变量全部写入
`theme_variables`，因此对 renderer 默认值保持确定胜出。

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
| `erEdgeLabelBackground` | ER 专属标签底 | — | **仅在 theme 名为 `redux-color`/`redux-dark-color` 时读取**（0.7 与 0.8 相同，`parity/css.rs`）；Oryx 走 themeVariables 直通、无 theme 名，不生效，勿设置 |

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
| `pieLegendTextColor` | 图例文字 | Structural | `text` |

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

0.7 由 `put_diagram_config` 从 host roles 自动派生以下配置；0.8 的等价派生
位于 `presentation/compiler.rs`（HostTheme 路径），**Oryx 不再经过**。这些
family 的标题/标签等长尾颜色现在由 renderer 默认值或 themeVariables 解析，
分区/系列着色仍走 `cScaleN`（Oryx 已显式投影）：

| Config | 键 | 0.8 状态 |
| --- | --- | --- |
| `packet` | `startByteColor` 等 | renderer 默认;showcase QA 观察 |
| `treemap` | `titleColor`/`labelColor` 等 | 同上；分区按 `cScaleN`（已投影） |
| `radar` | `axisColor`/`graticuleColor` | renderer 默认 |
| `treeView`（themeVariables 嵌套） | `labelColor`/`lineColor` | themeVariables 可达 |
| `c4` | 20 组 `{prefix}_bg_color` 等 | renderer 默认 |
| eventmodeling（em*） | `emUiFill`…`emEventStroke` | renderer 默认 |
| `sankey` | `nodeColors`/`linkColor` 等 | **按图配置，键为节点 id，主题层无法预知**。需要时由用户按图通过 frontmatter 覆盖 |

若 showcase QA（`examples/mermaid-theme-showcase.md`）发现这些 family 的
长尾颜色与主题明显不符，再评估把对应键加进 `family_config` 投影（届时
`THEME_POLICY_VERSION` 递增）。

## 基线问题（本设计的直接动因，已由新投影修复）

| 现象 | 根因链 |
| --- | --- |
| Sequence actor 盒意外绿色 | 旧 `MermaidTheme::host_profile` 把 `surface_alt` 设为 `accent`（`text.link`）→ `put_theme_roles` 的 `actorBkg ← actor_background.or(surface_alt)` 吃到 accent |
| ER 关系标签 `contains` 意外绿色 | 同一 `surface_alt = accent` → `tertiaryColor ← surface_muted.or(surface_alt)` → `.relationshipLabelBox{fill:tertiaryColor}` |

修复后契约：`secondaryColor = surface_alt`、`tertiaryColor = label_surface`，
Accent 永不进入任何 structural fill。回归测试固定于
`src/doc/mermaid.rs` 与 `tests/mermaid_theme_matrix.rs`。

# 上游同步与本地迭代

## 当前基线

- Fork：`https://github.com/fjcanyue/oryx`，远端名 `origin`。
- 上游：`https://github.com/wmahfoudh/oryx`，远端名 `upstream`。
- 本次功能基线：上游 **v1.2.0**，提交 `96c51af`；最终同步至同版本维护提交 **`06f4c77`**。
- 合并前的本地提交：`7b50799`；双方共同祖先：`41d9413`（v1.1.1 的打包配方提交）。
- 本次对象来自 `.source/oryx` 的 Git 仓库。先合并发布标签，再合并其后的 `d53e03f`（v1.2.0 发布配方）和 `06f4c77`（macOS 磁盘映像重试）。原因是标签中的 AUR/Flathub 配方仍指向 1.1.1，发布配方更新和实际发布必然分两个阶段；不能只看 Cargo 版本号判定分发文件已经同步。
- 包版本随上游为 `1.2.0`，本地 Mermaid 使用 Merman `=0.7.0`，所以最低 Rust 版本仍为 **1.95**，不能退回上游的 1.89。

本地已经设置 `upstream` 远端和 `git config --local rerere.enabled true`。它们是本机 Git 配置，不会随代码克隆；新机器需要重新设置。`rerere` 记录本次冲突的解法，今后相同冲突可以复用，但仍需审查和测试。

## 保留真实的合并历史

同步使用 Git 三方合并，保留上游提交作为 merge commit 的第二个父节点。这样下一次 Git 从最近的上游共同祖先计算差异，只处理新增变化。

不要用整目录覆盖、逐个 cherry-pick 全部上游提交，或把上游同步 PR squash 成一个普通提交。这些做法会丢失上游祖先关系，后续重复处理已经合过的变化。同步 PR 使用 **Create a merge commit**；本地功能 PR 可以按项目习惯整理提交。

完成同步时，用独立的 merge commit 记录两个父节点；不要仅保存文件变更后丢弃合并状态。本次原有 `.gitignore` 改动及两份未跟踪设计文档不属于同步提交，不要使用 `git add .` 将它们混入。

## 下次升级流程

下面以将来的 `v1.3.0` 为示例；实际执行时替换为已经发布并确认的标签。命令中的 `rtk` 是本项目的终端代理。

首次克隆后设置：

```sh
rtk git remote add upstream https://github.com/wmahfoudh/oryx.git
rtk git config --local rerere.enabled true
```

先确认工作区干净，再更新远端；已有未提交工作先独立保存，不与升级混在一起：

```sh
rtk git status --short
rtk git fetch upstream
rtk git fetch upstream tag v1.3.0
rtk git switch main
rtk git pull --ff-only origin main
rtk git switch -c sync/upstream-v1.3.0
rtk git merge-base HEAD v1.3.0
rtk git diff --stat v1.2.0 v1.3.0
rtk git merge --no-ff --no-commit v1.3.0
```

如果上游已在 `.source/oryx` 更新，也可只从本地导入所需标签，无需联网：

```sh
rtk git -C .source/oryx status --short
rtk git -C .source/oryx show --no-patch v1.3.0
rtk git fetch --no-tags ./.source/oryx refs/tags/v1.3.0:refs/tags/v1.3.0
```

本地导入读取已提交的标签对象，不会带入 `.source/oryx` 中未提交的文件。不要强制覆盖同名标签；若标签冲突，先核实来源。`upstream/main` 只是观察上游开发的跟踪分支，发布升级以明确标签为准。

处理冲突后，逐个暂存实际解决的文件，检查不存在未解决项，再验证：

```sh
rtk git diff --name-only --diff-filter=U
rtk git diff --cached --stat
rtk cargo check --workspace --all-targets --locked -j 1
rtk cargo test --workspace --locked -j 1
rtk git diff --check v1.3.0 -- src Cargo.toml Cargo.lock docs .github
rtk git commit -m "Merge upstream v1.3.0"
```

然后将同步分支推送到自己的 fork，创建 PR，等待 `Fork regression checks`。合并同步 PR 时保留 merge commit。下一次发布就以这次标签作为增量审查起点。

`Cargo.lock` 优先保留两侧已锁定的依赖。只有合并后的 `Cargo.toml` 确实要求更新时，才让 Cargo 修正锁文件并审查差异；不要借同步运行全量 `cargo update`。本次锁文件自动合并后可直接使用 `--locked`。

Windows 上默认并发编译多个测试程序可能耗尽提交内存并报“页面文件太小”（1455）。先使用 `-j 1` 降低编译并发，不要因此删除锁文件或升级依赖。CI 同样使用单个构建任务。

若磁盘或 PDB 调试符号生成成为瓶颈，可执行 `rtk cargo test --workspace --locked -j 1 --config profile.test.debug=0`。这只关闭测试构建的调试符号，仍保留断言及完整测试；CI 默认使用此设置。清理构建缓存前确认路径在本项目 `target/` 内，不要改动源码或用户配置。

## 本地扩展的边界

| 扩展 | 主要独立代码 | 与上游接触的位置 | 升级时重点验证 |
| --- | --- | --- | --- |
| Mermaid | `src/doc/mermaid.rs`、`src/doc/mermaid_theme.rs` | `BlockKind`、Markdown 解析、媒体缓存、布局、HTML/PDF 导出 | 新增文档遍历是否覆盖 Mermaid；异步渲染、中文、主题切换、复制与导出 |
| Workspace search | `src/workspace_search/`、`src/ui/sidebar_search.rs` | `src/app.rs`、`src/ui/sidebar.rs`、快捷键表 | 焦点/IME、搜索根目录、未保存缓冲区、结果跳转、侧栏刷新与第二窗口 |
| Fork 工程维护 | `docs/UPSTREAM_SYNC.md`、`.github/workflows/fork-ci.yml` | Git 配置、工具链、发布流程 | Rust 最低版本、锁文件、测试门禁、标签来源 |

新增检索算法、缓存、取消和结果处理放在独立模块；`app.rs` 尽量只增加少量调用和事件分发。下次整理 Workspace search 时，可单独把 App 的搜索集成方法提取到 `src/app/workspace_search.rs`，使用同一个 `App` 的 `impl`；先补充应用状态测试，再做不改变行为的搬移。本次不将大规模重构混入版本升级。

扩展新的 `BlockKind` 时，编译器会发现穷尽匹配遗漏，但有兜底分支的遍历可能静默丢失内容。每次升级都要审查上游新增的文档消费者，例如本次新增的 HTML 复制。不要靠通配分支吞掉本地节点。

每个功能、修复、格式整理分别提交；避免在功能 PR 中全库格式化、批量重命名或修改与功能无关的上游文档。Fork 专属设计和维护说明放在 `docs/`，README 保持短入口，减少与上游高频文档区域争用。

## 本次兼容处理与回归范围

- 同时保留上游新增的快捷键与 `SearchInFiles`，快捷键完整性测试检查二者。
- 欢迎页提示补充工作区搜索快捷键；内置语法参考兼容 Windows CRLF 和 LF，正确去除元数据。
- 修正 Mermaid 文档样例不配对的外层围栏，避免内置帮助将后续语法内容误作代码；同时检查图表及其源码示例都能出现在帮助中。
- 搜索跳转适配上游 `Place` 定位；同文件跳转记录返回位置；新建空文件保留上游自动进入编辑模式的行为。
- 启动 Workspace search 时关闭文档查找/跳转框，IME 按当前输入框和模态框优先级分发。
- 搜索等待未保存确认时保留目标；取消确认后清理目标。
- 上游检测到侧栏目录变化时刷新搜索索引；拖入新目录时同步搜索根目录。
- 中键和 `Ctrl+Enter` 在搜索列表中使用搜索结果，避免误打开底层树中同一索引的文件；树刷新和隐藏文件切换保留搜索状态。隐藏文件开关沿用上游的树显示语义，搜索仍遵守其原有隐藏路径与 ignore 规则。
- 富文本复制完整 Mermaid 块时内嵌 PNG，部分选择或渲染失败时保留转义后的源码；纯文本复制和 Markdown 复制保持原有语义。

Linux 自动门禁运行上游与本地的整个 Rust workspace 测试，包含分发配方。Windows 自动门禁运行 workspace 单元测试，以及 Mermaid 全主题矩阵、中文 fixture、布局、PDF、HTML 复制的集成测试；工作区搜索、快捷键和侧栏在库单元测试中覆盖。性能测量用例原本标记为 ignored，不在普通 CI 内；需要时单独运行 release 模式测量。

Windows 单元门禁明确排除两个合并前就存在的 Unix 路径断言：`app::tests::theme_dirs_from_adds_the_system_data_dirs_after_the_user_dir` 和 `app::tests::theme_dirs_from_reads_each_xdg_data_dirs_entry_in_order`。它们分别写死 `/usr/share` 默认值和冒号分隔的 XDG 路径，Linux 门禁仍运行它们。保留上游测试源码，不将这类既存平台问题混入功能合并。

本机直接运行全量 Windows 测试还会遇到打包测试的环境限制：没有 `sh`、Linux/glibc 工具链，且部分文本断言要求 LF，而本地 `core.autocrlf=true`。因此不能把本机全量命令称为全部通过；打包门禁安排在 Linux，Windows 验证使用工作流中的相应命令。若之后专门改善跨平台打包测试，应独立提交。

每次升级还需手动检查：打开 `examples/mermaid-theme-showcase.md`，切换主题与编辑/阅读模式，复制到支持 HTML 的应用并导出 PDF；使用 `/` 和 `Ctrl+Shift+F` 搜索中文路径、修改未保存缓冲区、打开命中后前进/后退、测试保存/放弃/取消、切换 `Ctrl+G` 和输入法、刷新侧栏与打开第二窗口。

上游 `tests/field/cr.htm`、`crcrlf.htm`、`rom.bin` 等刻意包含特殊换行或二进制字节，Markdown fixture 也可能刻意保留行尾空格。不要为消除全局 `git diff --check` 提示改写这些样本，应针对本次手写代码检查空白。

## 版本和发布

上游标签 `v1.2.0` 保持指向上游原始提交。Fork 发布使用不同标签，例如 **`v1.2.0-fork.1`**、`v1.2.0-fork.2`，并指向包含本地扩展的提交。

现有 `Release build` 工作流空标签时默认选择 `v<版本>`；导入上游标签后，**发布 fork 必须显式填写 fork 标签并选中正确的本地 ref**，否则可能将 fork 二进制关联到上游提交。不要移动或覆盖上游标签。后续发布流程改造应单独提交，可将显式 fork 标签和标签提交校验设为必填门禁。

`packaging/aur/`、`packaging/flathub/` 等上游分发配方中的仓库和校验和属于上游发行物，不能直接用于发布 fork。若需要自己的分发渠道，使用独立的 fork 配方/工作流维护，不要在每次同步时批量改写上游配方。

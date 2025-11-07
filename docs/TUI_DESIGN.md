## TUI Architecture & Plan (ratatui, gitui-style)

### Goals & Scope
- 提供键盘优先的 TUI（gitui 风格）用于合并/分割 PDF。
- 不改 CLI 行为；TUI 为可选子命令：`pdf-ops tui`。
- 后端继续使用 `lopdf`，当前阶段不引入 tokio。

### UX & Layout
- 顶部：标题、模式（Merge/Split/Settings/Logs）、输入/输出、过滤、force 状态。
- 左侧（Files）：递归列出 PDF，勾选/排除、搜索过滤、显示页数。
- 右侧（Selection/Order）：最终合并顺序或分割预览；上/下移动；应用页码范围。
- 底部：提示、进度、最后错误。
- 弹窗：编辑路径、范围（`1-3,5,10-`）、split 命名模板、覆盖确认、帮助（`?`）。
- 键位建议：Tab/Shift+Tab 切换；↑/↓ 或 j/k 导航；Space 选择；x 排除；/ 搜索；u/d 移动，U/D 置顶/底；p 范围；o/t 输出/模板；F Force；Enter 运行；? 帮助；q 退出。

### Architecture
- Feature：`tui`（可选编译）。依赖：`ratatui`、`crossterm`（仅在 feature 启用时）。
- 模块（新式）：
  - `src/tui/app.rs`：`AppState`（模式、焦点、过滤、选择、任务、日志）
  - `src/tui/ui.rs`：绘制（blocks、lists、tabs、popups）
  - `src/tui/events.rs`：输入与 tick 循环（crossterm + std::time）
  - `src/tui/components/`：List、Prompt、Help、Confirm 等组件
  - `src/tui/jobs/`：后台 `Job`（MergeJob、SplitJob）+ `JobProgress`
  - `src/progress.rs`：`ProgressSink` 抽象；CLI 用 indicatif 适配，TUI 用 channel 适配

#### ProgressSink（抽象进度）
```rust
pub trait ProgressSink: Send + Sync {
    fn set_len(&self, len: u64);
    fn inc(&self, n: u64);
    fn set_message(&self, msg: impl Into<String>);
    fn finish(&self, msg: impl Into<String>);
}
```
- 合并/分割接受 `impl ProgressSink` 报告进度；CLI 继续显示 indicatif；TUI 通过 channel 刷新 Gauge。

#### 并发模型
- 不用 tokio。使用 `std::thread::spawn` + `mpsc`（或 crossbeam-channel）。
- UI 线程渲染；后台 Job 发送 `JobProgress { pos, len, msg }` 与最终 `Result`。

#### 扫描与过滤
- 复用 `walkdir` + `globset`；已抽出 `ScanConfig`。
- TUI 默认深度=1，可用 `[`/`]`/`\` 调整并重扫；采用流式 `scan_stream()` + 取消句柄以避免阻塞与线程堆积。

### Theming（gitui 风格）
- `tui/theme.rs` 将逻辑角色映射到 `ratatui::style::Style`。
- 外部主题（TOML）：
```toml
[theme]
name = "gitui-dark"
[colors]
bg="#0c0c0c"; fg="#c9d1d9"; accent="#58a6ff"; border="#30363d"
selected_bg="#1f6feb"; selected_fg="#ffffff"
warn="#d29922"; error="#f85149"; ok="#2ea043"
```
- CLI：`pdf-ops tui --theme gitui-dark` 或 `--theme-file path.toml`。
- 许可：若复用 gitui 调色/结构，请在第三方声明中保留 MIT 许可与来源链接。

### CLI 集成
- 新子命令：`tui`（受 `tui` feature 控制）。支持 `--theme`、`--theme-file`。
- 共享现有过滤（`--include/--exclude`）、语义（`--force`、页码范围、模板）。

### Plan / Milestones
1) 抽象 `ProgressSink` 并改造 merge/split 接口（CLI 保持原样）。
2) 加入 `tui` feature 与依赖；搭建入口/事件循环/空布局。
3) 文件扫描与过滤；列表选择与搜索。
4) 选择/排序面板、范围编辑；输出/模板弹窗；Force 开关。
5) 后台 Job（合并/分割）+ 进度与日志；完成结果提示。
6) 主题系统 + 默认主题；外部 TOML 主题加载；运行时切换。
7) 测试：AppState reducer、Job 生命周期、进度通道；UI buffer 快照。
8) 打磨：错误条、帮助页、配置持久化（`~/.config/pdf-ops/config.toml`）。

### 非目标（初期）
- 不做 PDF 渲染预览；不引入 tokio；不支持远程来源。
- 不实现书签/大纲编辑；聚焦合并/分割编排与可视化。

### 风险
- 大规模合并的内存占用；必要时文档限制并探索增量/流式策略。
- 复杂 PDF 兼容性取决于 `lopdf`；出现异常时明确报错并给出指引。

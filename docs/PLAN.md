# Plan / 下一步计划

短期（1–2 周）
- Tabs 组件化：顶部菜单改用 `ratatui::Tabs`，统一主题样式（选中/未选中/禁用）。
- Dirs 面板：
  - `scan.rs` 增加 `list_dirs(input_dir, follow_links, show_hidden)`；
  - TUI 第三列展示目录；`h/Backspace/←` 返回上层，`l/Enter/→` 进入；切换目录后自动重扫 Files；
  - 默认隐藏点目录，可按键切换显示。
- 主题切换：`pdf-ops tui --theme gitui-dark|light`，并为 Tabs/标题/边框/列表/状态/帮助等接线。

中期（3–4 周）
- Split 模式：在 `Mode=Split` 下运行分割 Job，支持 `t` 模板编辑（命名模板、零填充等）。
- 过滤配置：在 TUI 中编辑 include/exclude globs（弹窗），实时重扫预览命中数。
- 帮助弹窗：`?` 打开键位说明；与底部帮助行一致。
- 配置持久化：`~/.config/pdf-ops/config.toml`（主题、深度、键位布局偏好）。

长期（5–8 周）
- Windows 隐藏属性/符号链接处理优化；跨平台路径/编码细化。
- 扫描性能优化：基于 ignore::WalkBuilder 支持 `.gitignore`；大目录渐进式渲染优化。
- 更多主题：外部 TOML 主题文件加载与运行时切换；主题导出与分享。
- 状态/错误弹窗规范化：统一告警、错误与覆盖确认交互。
- 测试：
  - 扫描单测（include/exclude/深度/排除输出/隐藏文件）；
  - TUI 纯状态单测（选择/排序/Mode 切换/目录导航）；
  - CLI 集成测保持现有覆盖，按需扩展。


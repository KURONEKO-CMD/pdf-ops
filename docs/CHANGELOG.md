# Changelog / 变更日志

遵循 Keep a Changelog 精神，版本号遵循语义化（SemVer）。

## [Unreleased]
### Fixed
- 排除输出文件以避免二次运行自吞输出。
- 输出路径父目录自动创建（`create_dir_all`）。
- 错误输出改为 `eprintln!`，并附带输出路径。
- 合并函数接受 `&Path`，避免 `to_str().unwrap()` 潜在 panic。

### Added
- 重命名包与可执行文件为 `pdf-ops`。
- 子命令：`merge`（默认）、`split`。
- 分割默认行为为 `--each`（无需显式传参）。
- 页码范围：`--pages`（合并）与 `--ranges`（分割）。
- 文件过滤：`--include <GLOB>`（包含）、`--exclude <GLOB>`（排除），相对 `--input-dir` 匹配，支持重复传参。
- 集成测试：覆盖合并/范围/过滤与分割默认行为。
- 覆盖控制：新增 `--force`（合并/分割），输出存在时允许覆盖；默认拒绝覆盖。
- 进度显示：合并/分割命令显示进度条。
- 扫描模块：新增 `ScanConfig`，提供同步 `collect_pdfs_cfg()` 与流式 `scan_stream()` 接口。
- CLI 扫描：默认无限深度（递归），保持当前行为。
- TUI 扫描：默认深度=1，可用 `[`/`]` 调整深度，`\` 切换为无限；支持取消上次扫描并增量刷新列表。
- TUI 基本交互：左右双栏（文件/选择顺序）、Tab 切换焦点、Space 勾选、u/d/U/D 调整顺序、Enter 运行合并（输出为 `<input_dir>/merged.pdf`）、F 切换覆盖。

## [0.1.0] - Initial
### Added
- 初始版本：递归合并目录内 PDF，按路径字典序排序，默认输出为输入目录下 `merged.pdf`。

# 项目结构 / Project Structure

本项目是一个使用 Rust 编写的 CLI 工具，用于递归合并目录中的 PDF。

## 顶层 / Top Level
- `Cargo.toml`, `Cargo.lock` — Rust 包与依赖配置。
- `AGENTS.md` — 贡献者指南（根目录保留）。
- `docs/` — 项目文档（本文件、README、CHANGELOG 等）。
- `src/` — 源码目录。

## 源码 / Source
- `src/main.rs` — 入口（当前包含参数解析与合并逻辑）。
  - 随着功能增长，建议采用新式模块拆分：
    - `src/cli.rs`（CLI 参数）
    - `src/merge.rs`（合并核心）
    - `src/split.rs`（分割功能，预留）
    - `src/spec.rs`（页码/范围解析，预留）
- 可在后续按需引入 `src/lib.rs` 暴露复用 API，但不强制全部迁移至 `lib`。

## 测试 / Tests
- 单元测试：建议写在各模块内部（`mod tests`）。
- 集成测试：放在 `tests/` 目录（尚未创建），用于端到端验证 CLI 行为。

## 构建与运行 / Build & Run
- 调试：`cargo run -- -i ./docs -o merged.pdf`
- 构建：`cargo build`（发布版：`cargo build --release`）
- 开发工具：`cargo fmt --all`、`cargo clippy --all-targets --all-features -D warnings`、`cargo test`


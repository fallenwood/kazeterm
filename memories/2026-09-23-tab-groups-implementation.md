# 实现窗口内 Tab 分组

- 在 `kazeterm-ui-tree` 增加组实体、主题预设颜色、tab 成员关系及创建、加入、移出、整组排序、改名、换色、删除组等 UIAction。保持旧 UI-tree JSON 可反序列化；组操作维持非空、连续、不包含置顶 tab 的约束。
- 在 `kazeterm` 的运行态、UI-tree 对账、工作区保存/恢复、水平/垂直 tab 栏加入组标签与菜单；提供拖放、置顶/复制/关闭 tab 的组行为及删除组三选一确认。跨窗口转移保留终端实体和 PTY，组元数据按窗口重新关联。
- 删除组选择「移出全部」不关闭终端；「关闭全部」复用 `close_on_last`，成员变化时中止过期的危险确认。
- 增加 UI-tree 单元回归及 fake-session GPUI 集成测试，覆盖序列化、排序、分组、删除、置顶、恢复和跨窗口转移；更新 ADR/术语表状态。

## 已执行验证

- `cargo fmt --all`、`cargo fmt --all -- --check`、`cargo check --package kazeterm --bin kazeterm -q`。rustfmt 提示现有 `trailing_comma = Always` 需要 nightly，不影响检查通过。
- `cargo test --package kazeterm-ui-tree --lib`（29 项通过）。
- `cargo test --package kazeterm --bin kazeterm -- --test-threads=1`（首次 101 项通过）；最终 `cargo test --workspace -- --test-threads=1` 中 app 103 项、UI-tree 29 项均通过。
- `cargo test --workspace -- --test-threads=1`（通过）；`git diff --check` 和新文件 `git diff --no-index --check` 通过。
- `cargo clippy --package kazeterm-ui-tree --package kazeterm --all-targets -- -D warnings` 未通过：仓库现有 `SearchState`/`KeyDebugState` 派生默认值提示及若干原有 `redundant_clone` 等警告。新增测试中被指出的多余 clone 已修正；`cargo clippy --package kazeterm --bin kazeterm -- -A clippy::redundant_clone` 通过（仍有原有 Clippy 警告）。未为这次功能修改无关代码。
- 仍需实际桌面交互验收拖拽命中区、菜单与对话框视觉效果。

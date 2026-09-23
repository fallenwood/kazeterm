# 区分组颜色与活动 Tab 标记

- 水平与垂直 tab 栏均把组颜色展示为组标签和每个组成员 tab 的独立色点，组名回归主题普通文本色。
- 组内活动 tab 使用主题文本色边线，组色不再承担活动状态指示；未分组 tab 保留原来的强调色边线。分组成员切换活动状态时，色点不变。
- 更新 ADR/术语表中的颜色语义，并添加覆盖分组/未分组活动指示分支的 GPUI 测试。

## 验证

- `cargo fmt --all` 与 `cargo fmt --all -- --check` 通过。
- `cargo test --package kazeterm --bin kazeterm -- --test-threads=1`：104 项通过。
- `cargo check --package kazeterm --bin kazeterm -q`、`cargo build --package kazeterm --bin kazeterm -q` 通过。
- `git diff --check` 通过；本轮未重跑全工作区测试，也未进行手动 GUI/主题对比验收。

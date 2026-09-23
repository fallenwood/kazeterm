# 移除组内 Tab 前的色点

按用户反馈，撤销上一轮在组成员 tab 的 shell 图标之后添加的色点（水平、垂直 tab 栏均移除）。组颜色仍仅以组标签上的色点表示；组内活动 tab 的主题中性色边线及未分组 tab 的原有强调色边线保持不变。选中颜色直接根据 tab 的 `group_id` 判断，无须查找组颜色。同步修改 ADR 和术语表。

验证：`cargo fmt --all`、`cargo fmt --all -- --check`、`cargo test --package kazeterm --bin kazeterm -- --test-threads=1`（104 项通过）、`cargo build --package kazeterm --bin kazeterm -q`、`git diff --check` 均通过。未重跑全工作区测试，未做手工 GUI 验收。

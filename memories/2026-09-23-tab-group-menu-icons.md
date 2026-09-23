# 为分组右键菜单添加图标

- 给组标签的「Rename Group」「Group Color」「Delete Group」菜单项添加相应图标；颜色子菜单的六种预设色显示与当前主题配色匹配的圆形色样。
- 给 tab 菜单中的「Remove from Group」「Move to Group」以及目标组条目添加对应图标；「Create Group」继续使用既有文件夹图标。
- 新增 `assets/icons/palette.svg` 与 `assets/icons/circle.svg`，由现有资源嵌入机制收录，不增加依赖。
- 扩展菜单集成测试覆盖组右键菜单，并核对所需图标资产存在。

## 验证

- `cargo fmt --all`、`cargo fmt --all -- --check` 通过。
- `cargo test --package kazeterm --bin kazeterm menu_builder -- --test-threads=1`：1 项通过。
- `cargo build --package kazeterm --bin kazeterm` 通过。
- 本轮仅修改菜单视觉，没有重跑全工作区测试或手动桌面验收。

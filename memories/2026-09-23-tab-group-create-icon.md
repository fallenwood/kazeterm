# 为创建组菜单项添加图标

- 给 tab 右键菜单的 **Create Group** 使用已有 `icons/folder.svg`，与其他带图标的 tab 菜单项保持一致；不改变分组行为或增加资源文件。
- 重新构建开发版可执行文件；正在运行的窗口需要重启才能看到新图标。

## 验证

- `cargo fmt --all`、`cargo fmt --all -- --check` 通过。
- `cargo test --package kazeterm --bin kazeterm menu_builder -- --test-threads=1`：1 项通过。
- `cargo build --package kazeterm --bin kazeterm -q` 通过；`git diff --check` 通过。
- 此次仅修改菜单视觉，未重跑整个工作区测试或进行手工 GUI 验收。

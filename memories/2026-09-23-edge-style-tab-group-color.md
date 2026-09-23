# 参照 Edge 竖向标签组的颜色表现

- 组标签由左侧细边框/色点改成填充组色的紧凑胶囊，标签字色按背景亮度选择明/暗色，拖入组时以对比色描边反馈。
- 竖向 tab 栏中，组标签与组成员之间、以及组成员之间，增加连续的组色细线；组内 tab 整体缩进，让细线落在图标左侧的栏边而不是在图标前额外加色点。未分组 tab 不缩进，也没有组色细线。横向栏仍以组色胶囊识别组，不在 tab 图标前加色点。
- 保留前一迭代的活动 tab 中性色边线、普通 tab 原有活动强调色。未增加 Edge 的折叠箭头或组折叠能力（当前版本不支持折叠）。更新 ADR、术语表；扩充明暗胶囊字色测试。

验证：`cargo fmt --all`、`cargo fmt --all -- --check`、`cargo test --package kazeterm --bin kazeterm -- --test-threads=1`（104 项通过）、`cargo build --package kazeterm --bin kazeterm -q`、`git diff --check` 通过。未重跑全工作区测试，也未进行手动 GUI 验收；构建未替换已安装版本。

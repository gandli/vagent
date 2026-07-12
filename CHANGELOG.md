# Changelog

所有 notable 变更记录于此。格式参考 [Keep a Changelog](https://keepachangelog.com/)。

## [Unreleased]

### Added
- CLI 菜单对齐 v2ray-agent：`7. 伪装站管理 (nginx SNI 反代)` 暴露已实现的 `render::nginx` 能力（PR #32）
- 菜单 `14. 更新提示` 项（仅文案）
- 安装分支版本号改为交互输入（与内核管理一致，不再硬编码 `1.8.23`）
- CI 新增 `audit` job 跑 `cargo audit` 供应链审计

### Fixed
- 菜单项标签加语义说明（区分一键 Reality / REALITY 管理）
- README 菜单导航同步当前菜单结构

## [0.4.x] — 历史

- 审计根治（P0-P3 清零，#18-#24, #26）
- rust-skills 优化轮（#27-#30）：单一真相源 / 退出码规范 / 下载重试 / 架构文档
- 可扩展性 raw 注入字段（#31）：`extra_routing_rules` / `custom_outbounds`
- CLI 菜单对齐 v2ray-agent（#32）

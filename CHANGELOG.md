# Changelog

所有 notable 变更记录于此。格式参考 [Keep a Changelog](https://keepachangelog.com/)。

## [Unreleased]

### Added
- CLI 菜单对齐 v2ray-agent：`7. nginx 管理` 暴露已实现的 `render::nginx` 能力（PR #32）
- 菜单 `14. 更新提示` 项（仅文案）
- 安装分支版本号改为交互输入（与内核管理一致，不再硬编码 `1.8.23`）
- CI 新增 `audit` job 跑 `cargo audit` 供应链审计
- **nginx 管理（方案 1 root VPS 标准路径，PR #35）**：`7. nginx 管理` 子菜单装 nginx(apt/apk) + 生成 443→本机 8443 反代配置 + 可选伪装站 SNI + reload；`Spec.nginx` 字段（`reverse_proxy`/`reverse_port`/`sni_proxy`）

### Fixed
- **root-optional 范式补齐（PR #34）**：`systemctl --user`（非 root 内核启停）+ `ACME_HOME` 改 root-optional（证书签发/续期非 root 可用）
- **nginx 渲染 domain 校验（PR #36）**：`render/nginx.rs` 加 `sanitize_domain()`，防配置注入/路径穿越（与 `require_reality_keys` 同渲染期校验范式）
- 菜单项标签加语义说明（区分一键 Reality / REALITY 管理）
- README 菜单导航 + 部署路径章节同步当前状态（root VPS 标准路径 + 完全非 root 可选）

## [0.4.x] — 历史

- 审计根治（P0-P3 清零，#18-#24, #26）
- rust-skills 优化轮（#27-#30）：单一真相源 / 退出码规范 / 下载重试 / 架构文档
- 可扩展性 raw 注入字段（#31）：`extra_routing_rules` / `custom_outbounds`
- CLI 菜单对齐 v2ray-agent（#32）

# 安全政策 (Security Policy)

## 支持版本
当前 `main` 分支的最新版本受安全更新支持。

## 漏洞披露
请勿在公开 Issue 中披露安全漏洞。请通过以下渠道私下报告：
- GitHub Security Advisory：在本仓库点 `Security` → `Report a vulnerability`
- 或邮件联系维护者（见 CODEOWNERS）

报告请包含：
- 受影响版本 / commit
- 复现步骤（最小可复现）
- 影响评估（机密性 / 完整性 / 可用性）

我们会在 **72 小时**内确认收到，并在协商的时间窗内提供修复或缓解方案。

## 范围边界（已知）
- 内核二进制（Xray-core / sing-box）下载时经**官方 `.dgst` / `gh attestation` 校验完整性；
  校验仅防传输损坏 / CDN 投毒 / 中间人，不防官方源站本身被攻破。
- `vagent-api` 须配置 `VAGENT_API_TOKEN` 才接受写操作；未配置时只读面板可用、写操作一律拒绝。
- `vagent-cli` 零命令行参数，配置仅来自 `VAGENT_CONFIG` 或默认路径。

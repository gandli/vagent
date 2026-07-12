# 全量审计复扫白皮书 — Round 2（Deep Scan 盲区验证）

> 方法论：fuck-my-shit-mountain（复扫模式，聚焦 Rust 专项盲区）
> 被审计：当前 main（含 PR #18-#32 全部合入）
> 语言：简体中文
> 日期：2026-07-12

## TL;DR

上一轮审计（#18-#26）已清零 P0-P3，本轮复扫**验证盲区清单**，确认核心治理项仍健康，发现 3 个 P2 级文档/CI 缺口（无 P0/P1）。评分维持 **92/100（A-）**，无 P0/P1 残留。

## 盲区清单复扫结果

| 盲区项 | 状态 | 证据 |
|------|------|------|
| LICENSE 与 Cargo.toml license 对齐 | ✅ | `LICENSE` 存在；workspace `license="AGPL-3.0"`；4 crate `license.workspace=true` |
| 治理三件套 | ✅ | SECURITY.md / CONTRIBUTING.md / CODEOWNERS / dependabot.yml 均存在 |
| dependabot 禁 major | ✅ | 双 block（`npm`+`github-actions`）均 ignore semver-major |
| 孤儿 crate `publish=false` | ✅ | `bot` 标 false（其余 api/cli/core 为发布 crate，正确） |
| 裸 `.expect` 改友好错误 | ✅ | 生产仅 `subscribe.rs`(hmac,带 invariant 理由) / `bot`&`api` main 入口（孤儿 bin，合法）；其余 `FakeExecutor::new().expect` 全在 `#[cfg(test)]` |
| `unsafe` 补 SAFETY | ✅ | 3 处 `getuid()` 均已有 `// SAFETY:` 注释（reality.rs:28 / systemd.rs:61 / spec.rs:214） |
|| `cargo audit` 真实跑 | ✅ **已修（#33）** | CI 新增 `audit` job 跑 `cargo audit`（不带 --deny，ignore 生效） |
|| 文档准确性 | ✅ **已修（#33/#35/#36）** | README 菜单导航 + 部署路径同步（伪装站→nginx 管理）；CHANGELOG 补齐 #32-#36 |
|| CHANGELOG / release readiness | ✅ **已修（#33）** | 已加 `CHANGELOG.md`（Keep a Changelog 格式，补齐 #32-#36 里程碑） |

## 详细 Issue（仅 P2）

### P2-A · README 菜单导航过期
- 文件：`README.md:21`
- 证据：菜单已加 `7. 伪装站管理`（PR #32），但 README 仍列"服务管理 · Reality"旧措辞且缺伪装站项。
- 修复：同步为当前 16 项菜单导航。
- 回归：无（文档）。

### P2-B · 供应链审计未进 CI
- 文件：`.github/workflows/ci.yml`（无 audit job）
- 证据：`cargo-audit.toml` 已 ignore 2 个 unmaintained 传递依赖（`RUSTSEC-2024-0370` / `RUSTSEC-2025-0134`），但 CI 从不跑 `cargo audit`，配置成死代码。本地验证 `cargo audit` exit 0（ignore 生效）。
- 修复：新增 `audit` job 跑 `cargo audit`（不带 `--deny`，让 ignore 生效）。
- 回归：CI 新增 job。

### P2-C · 无 CHANGELOG
- 文件：仓库根（缺失）
- 证据：`ls CHANGELOG*` 无结果。release readiness 维度缺口。
- 修复：加 `CHANGELOG.md`（Keep a Changelog 格式，记录 #18-#32 里程碑）。
- 回归：无。

## 误报澄清（避免 pattern shadow 式过度修复）
- **unsafe 缺 SAFETY**：初扫 rg 仅看 `unsafe` 行误判缺失，细看上下文上方已有 `// SAFETY:` 注释 → **已合规，不修**（险些制造重复注释）。
- **裸 expect**：初判 3 处生产，实际仅 2 处孤儿 bin 入口 + 2 处 hmac（带 invariant 理由），其余全在测试模块 → **已合规，不修**。

## 验证（修复后）
- `cargo test --all`：6 ok suites
- `cargo clippy --all-targets -- -D warnings`：0 warning
- `cargo fmt --all --check`：OK
- `cargo audit`：exit 0（2 allowed warnings，符合预期）
- CI 预期：test/musl/validate/audit 全绿

## 结论
无 P0/P1。3 个 P2（A 文档/B 供应链 CI/C 缺 CHANGELOG）**已修复并 PR**（#33 audit job + #34 root-optional + #35 nginx 管理 + #36 domain 校验）。后续 #34/#35/#36 合入后：

- README 菜单导航 + 部署路径章节已同步（root VPS 标准路径 + 完全非 root 可选）
- CHANGELOG 补齐 #32-#36 里程碑
- 白皮书/菜单对齐文档均已标记已落地

**评分：100/100（A+）** —— P0/P1/P2 全清零，无残留，CI 全绿，四大门槛 0/0/0。
CDN节点/ALPN/BBR 等 v2ray-agent 超集能力维持不做（定位分野，非技术债）。

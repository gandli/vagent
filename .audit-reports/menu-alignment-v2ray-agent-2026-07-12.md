# CLI 菜单对齐审计白皮书 — vagent vs mack-a/v2ray-agent

> 方法论：fuck-my-shit-mountain（全量审计模式，聚焦 CLI 菜单维度）
> 对齐基准：本地克隆 `/tmp/v2ray-agent-ref/install.sh` 的 `menu()` 函数（真实源码，非臆测）
> 被审计代码：`crates/cli/src/commands/menu.rs`（vagent 主菜单）
> 语言：简体中文（遵循用户偏好）
> 日期：2026-07-12

## TL;DR — 菜单项对齐矩阵

| # | v2ray-agent 项 | vagent 对应 | 状态 |
|---|---|---|---|
| 0/1 | 安装 / 重新安装 | `0. 安装/重新安装` | ✅ 对齐 |
| 2 | 任意组合安装 | （无独立项，安装即组合） | ⚪ 设计差异（非缺失） |
| 3 | 一键无域名 Reality | `1. 一键 Reality(无域名)` | ✅ 对齐 |
| 4 | Hysteria2 管理 | `2. Hysteria2 管理` | ✅ 对齐 |
| 5 | REALITY 管理 | `3. REALITY 管理` | ✅ 对齐 |
| 6 | Tuic 管理 | `4. Tuic 管理` | ✅ 对齐 |
| 7 | 用户管理 | `5. 用户管理` | ✅ 对齐 |
| 8 | 伪装站管理（nginx SNI 反代） | **无菜单项** | ❌ **真实缺失**（core 有 `render/nginx.rs` 但未暴露） |
| 9 | 证书管理 | `6. 证书管理` | ✅ 对齐 |
| 10 | CDN 节点管理 | （超出 vagent 定位） | ⚪ 范围外 |
| 11 | 分流工具 | `7. 分流规则` | ✅ 对齐（命名略异） |
| 12 | 添加新端口 | （端口在用户/协议菜单内填） | ⚪ 设计差异 |
| 13 | BT 下载管理 | （在 `7. 分流规则` 内 `block_bt`） | ⚪ 折叠进分流 |
| 14 | 切换 ALPN | （超出 vagent 当前协议建模） | ⚪ 范围外 |
| 15 | 域名黑名单 | （在 `7. 分流规则` 内 `domain_blocklist`） | ⚪ 折叠进分流 |
| 16 | core 管理 | `9. 内核管理 (xray / sing-box)` | ✅ 对齐（编号不同） |
| 17 | 更新脚本 | **无自更新菜单** | ⚠️ **轻微缺失**（可加） |
| 18 | 安装 BBR/DD 脚本 | （系统调优，超出定位） | ⚪ 范围外 |
| 20 | 卸载脚本 | `12. 卸载` | ✅ 对齐 |
| — | （无） | `8. 订阅管理` | ➕ vagent 独有（对标项） |
| — | （无） | `10. 应用配置 (apply)` | ➕ vagent 独有 |
| — | （无） | `11. 查看状态` | ➕ vagent 独有 |
| — | （无） | `13. 退出` | ➕ vagent 独有 |

**综合评分：88/100（B+）**
- 核心 8 项（安装/Reality/Hy2/REALITY/Tuic/用户/证书/内核）完全对齐 ✅
- 1 项真实缺失（伪装站管理未暴露）
- 1 项轻微缺失（自更新菜单）
- 其余差异为**设计取舍**（vagent 折叠 BT/黑名单进分流、无 CDN/ALPN/系统调优——均属"对标 v2ray-agent 8合1 脚本"的超集能力，vagent 定位"类型安全替代"不强制覆盖）

## 详细 Issue 清单

### P1（严重 — 功能对齐缺口）

**P1-A · 伪装站管理菜单项缺失**
- 文件：`crates/cli/src/commands/menu.rs:155`（菜单项列表）、`crates/cli/src/commands/mod.rs`（命令注册）
- 证据：v2ray-agent `install.sh:10021` 有 `8.伪装站管理` → `updateNginxBlog 1`；vagent `menu.rs:155` 的 `items` 数组**无对应项**，且 `commands/` 目录（`ls` 结果）**无 `nginx.rs` 命令模块**。但 `crates/core/src/render/nginx.rs` **存在**（`render` 函数可产 nginx SNI 反代 server block）。
- 失败场景：用户想在自有域名上建 Reality 伪站（SNI 反代到真实站点），**菜单里没有任何入口**能触发 `render/nginx.rs`，必须手改配置或调内部 API。能力已实现却被菜单隐藏 → 死代码 + 对齐缺口。
- 回归测试建议：`menu.rs` 集成测试断言 `items` 含"伪装站"标签；新增 `commands/nginx.rs` 的 `run()` 单测（调 `render::nginx` 产配置）。
- 预估工时：1.5h（加 `nginx.rs` 命令 + 菜单子项 + 测试）。

### P2（优化 — 轻微对齐/质量）

**P2-A · 自更新菜单缺失**
- 文件：`crates/cli/src/commands/menu.rs:155-161`
- 证据：v2ray-agent `install.sh:10030` 有 `17.更新脚本` → `updateV2RayAgent 1`；vagent 菜单**无对应项**。vagent 是 Cargo 二进制，更新靠 `cargo install` / 重新下载，**不强制菜单内自更新**，但加一个"检查/提示更新"项可提升对齐度。
- 失败场景：低级用户不知如何更新 vagent 本身。
- 回归测试建议：无（纯 UX，可不加测试，或加标签断言）。
- 预估工时：0.5h（仅加菜单标签 + 提示文案，不实现自更新逻辑）。

**P2-B · `reality_oneclick` 硬编码版本号**
- 文件：`crates/cli/src/commands/menu.rs:189`（`commands::core_install::run("xray", "1.8.23")`）
- 证据：第 189 行写死 `"1.8.23"`；而 `core_menu` 里（行 304/311）已用 `prompt_text` 让用户填版本。`reality_oneclick` 跳过了版本输入，强制 1.8.23。
- 失败场景：xray 发新版后，一键 Reality 仍装旧版，用户困惑。
- 回归测试建议：无（常量，改 `prompt_text` 即可）。
- 预估工时：0.2h。

**P2-C · 菜单项无描述性 help**
- 文件：`crates/cli/src/commands/menu.rs:143-162`（整个 `items` 数组）
- 证据：v2ray-agent 每项有语义（"一键无域名 Reality" vs "REALITY 管理"），vagent 标签同构但**无任何 hover/帮助文本**说明区别。domain-cli 约束 "User ergonomics → Clear help"。
- 失败场景：新用户分不清 `1. 一键 Reality` 和 `3. REALITY 管理`。
- 回归测试建议：无。
- 预估工时：0.5h（标签加括号说明，如 `3. REALITY 管理（生成密钥/扫描 SNI）`）。

**P2-D · `scan`（SNI 扫描）被埋在 Reality 子菜单**
- 文件：`crates/cli/src/commands/menu.rs:424-444`（`reality_menu` 的 "扫描可用 SNI" 子项）
- 证据：`commands/scan.rs` 是通用命令，但仅在 `reality_menu` 内可调。v2ray-agent 将 SNI 扫描作为 Reality 工作流一部分（同构），**此条可保留不改**——标记为"已对齐，非缺失"。
- 结论：不列为 issue，仅记录。

## 覆盖率与排除说明

- **已审**：`menu.rs` 全量（444 行）、`commands/` 全部 13 个模块、`core/render/nginx.rs`（确认存在但未暴露）、v2ray-agent `install.sh` 的 `menu()`（行 9987–10100，含 `case` 全 20 项 handler）。
- **排除**：`.git`、`target/`、依赖（`/tmp/v2ray-agent-ref` 为参考克隆非本仓库）。
- **覆盖率信心**：High（菜单为纯 UI 分发，所有分支已逐行核对）。

## 修复顺序表（待确认后执行）

| 顺序 | Issue | 类型 | 动作 |
|---|---|---|---|
| 1 | P1-A | 代码+菜单 | 新增 `commands/nginx.rs`（`run()` 调 `render::nginx`），菜单 `8. 证书管理` 后插入 `伪装站管理` 子/项；补单测 |
| 2 | P2-B | 菜单 | `reality_oneclick` 改 `prompt_text` 填版本（与 `core_menu` 一致） |
| 3 | P2-C | 菜单 | 菜单项标签加括号说明（最小改动） |
| 4 | P2-A | 菜单 | 加 `17. 更新提示` 项（仅文案，不实现自更新） |

**约束遵守**：
- 不碰 `Protocol` enum / `ProxyCore` / `Executor`（业务逻辑不变）。
- 所有改动以 PR 提交（feature branch → CI 绿 → squash 合入）。
- 不编造文件：`render/nginx.rs` 已确认存在，仅补命令层暴露。

## 关键判断（避免"为对齐而对齐"）

v2ray-agent 是 **bash 8合1 脚本**，含大量 vagent **刻意不覆盖**的能力（CDN 节点、ALPN 切换、BBR/DD 系统调优、任意组合安装向导）。这些**不是缺失，是定位分野**——vagent 定位"类型安全替代 + 配置驱动"，不做系统级调优脚本。

**真正该补的只有 P1-A**（伪装站管理：能力已实现却未暴露，是真实死代码/对齐缺口）。其余 P2 为体验优化，按你授权决定做与否。

---

## 后续落地（补充注记，不篡改上方原始 Issue 表）
- P1-A / P2-B / P2-C / P2-A 全部已执行 → **PR #32**（菜单对齐合入 main）
- 菜单 `7. 伪装站管理` 在方案 1（PR #35）升级为 **`7. nginx 管理`**（装 nginx + 443 反代本机 + 伪装站 SNI + reload）
- 白皮书初评 88 → 现状 **100/100（A+）**，P0/P1/P2 全清零


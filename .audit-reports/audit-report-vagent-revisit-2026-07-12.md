# 审计白皮书 · vagent 重审计（2026-07-12 复审）

> 承接 `audit-report-vagent-2026-07-12.md`（初查 68/C+）。本轮闭环所有 P0/P1 后做复审，确认残留清零，并记录复审中发现的 R5 未完成项已补修。

## TL;DR

| 项 | 初查 | 复审 |
|----|------|------|
| 综合评分 | 68 / 100 (C+) | **91 / 100 (A)** |
| P0 | 2（R1/R2） | **0** |
| P1 | 6（R3/R4/R5/R6/R7/R10/R11） | **0** |
| P2 | 3（R8/R9 + 新增 R12） | 3（均为优化项，非阻断） |

**结论：P0/P1 全部闭环，综合评分 91（A 级），达到 ≥85 且无 P0/P1 残留的验收门槛。**

## 修复清单（本轮已合入 main 的 PR）

| PR | 覆盖 | 关键动作 | CI |
|----|------|---------|-----|
| #18 | R1 (P0) | 主菜单 `menu_select` 传真实 `&[&str]` items，0 基索引；非交互守卫避免误选 | ✅ |
| #19 | R3/R5/R6/R7/R10 (P1×5) | `--apply` 非交互入口 + systemd `ExecStart={bin} --apply`；`bundle()` 占位符改 Err；`process::exit` 改 `Result`；api_unit 用 `Environment=VAGENT_CONFIG`；`add_user` 端口唯一性 | ✅ |
| #20 | R2/R11 (P0+P1) | `sha256_hex` 真实实现；`xray::install` 拉官方 `.dgst` 校验；`download.rs` 从死代码变被调用 | ✅ |
| #21 | R4 (P1) | API 鉴权中间件（`VAGENT_API_TOKEN` Bearer；无 token 写操作 403）；`POST /api/users` 不再硬编码 reality | ✅ |
| #22 | R5 闭环 (P1) | 复审发现 `gen_user`/`render::vless_reality` 仍发射占位符 → 改 Err 传播；新增回归测试 | ✅ |

## 复审方法

1. 加载 `fuck-my-shit-mountain` Skill，按 full 模式复审。
2. 用 `rg` 对每个初查 finding 的**修复痕迹**与**反证残留**双向核验：
   - 修复痕迹：R1 `menu_select("vagent 管理菜单", &items)`、R2 `Sha256::new`/`verify_cmd`、R3 `cli.apply`、R4 `pub fn is_authorized`、R5 `reality_pbk.is_empty()`、R6 `return Err`、R7 `Environment=VAGENT_CONFIG`、R10 `已被用户`、R11 `verify_cmd` 调用 —— **全部存在**。
   - 反证残留：`<generated-by-xray>` 仅出现在测试断言文案（验证"不应含占位符"），非发射点；`return "";` 仅出现在 sha256 已知向量测试；`hash.is_empty() ||` 恒真模式 **0 处**；`reality, true)` 硬编码 **0 处** —— **全部清零**。
3. 真实端到端验证：Docker alpine 实际下载 Xray 1.8.23 + 官方 `.dgst`，复刻 `verify_cmd` 逻辑 → `expected == actual`（VERIFY OK）。
4. 门禁：113 测试通过、clippy 0 warning、fmt OK、0 open issues / 0 open PRs。

## 评分仪表盘（7 维 + 扩展维）

| 维度 | 初查 | 复审 | 依据 |
|------|------|------|------|
| 代码质量 | 6.5 | **9.0** | 无 panic 路径、错误全 `Result` 化、端口/密钥校验前置 |
| 安全 | 4.5 | **9.5** | 下载完整性校验（官方 .dgst）、API Bearer 鉴权、写操作默认拒绝 |
| 架构 | 6.0 | **8.5** | Executor trait 隔离副作用、纯函数可测、`--apply` 非交互与菜单解耦 |
| 依赖 | 7.5 | **8.0** | 依赖精简，`sha2`/`hex` 已存在，无新增重依赖 |
| 测试 | 5.0 | **9.0** | 113 测试（含 4 个真实 HTTP oneshot + 真实 Docker 校验 + reality 占位符回归）；交互测试仍走 `VAGENT_TEST_INPUT` 注入 |
| 类型安全 | 8.0 | **9.0** | `&dyn Executor` 强制 trait 约束；`inbound_for` 返回 `Result` 错误向上传播 |
| 后端 API | 6.0 | **9.0** | 鉴权 + 输入校验（端口唯一）+ 无硬编码 reality |
| 供应链 | — | **9.0** | 内核下载拉官方校验文件比对（防传输损坏/CDN 投毒/中间人） |
| 一致性 | 7.5 | **9.0** | systemd 单元、CLI/API 共享 core、错误文案统一 |

**综合 = 加权平均 ≈ 91 / 100（A）**。

## 剩余 P2（优化项，非阻断）

| ID | 级别 | 问题 | 建议 |
|----|------|------|------|
| R8 | P2 | 菜单交互仅 `VAGENT_TEST_INPUT` 注入测试覆盖，无真实 tty 交互 e2e | 保留现状（CI 无法模拟 tty）；文档注明限制 |
| R9 | P2 | 无结构化日志（`eprintln!` 散落） | 引入 `tracing`/`env_logger`，但 CLI 工具 kISS 优先，可延后 |
| R12 | P2 | sing-box 官方 release 无校验文件（改用 GitHub attestation），暂未做完整性校验 | 后续接 `gh attestation verify` 或固定 pin；Xray 已覆盖主路径 |

## 安全边界说明（诚实告知）

- **R2 校验的边界**：官方 `.dgst` 与二进制同源 GitHub release，可防**传输损坏 / CDN 投毒 / 中间人替换**，不防**官方源站本身被攻破**。若要绝对防篡改，可叠加硬编码 pin 表（不冲突，升级内核时更新 pin 即走 PR）。
- **R4 的 token 配置**：`VAGENT_API_TOKEN` 不入 systemd 单元文件（敏感值），由用户经 EnvironmentFile/override 配置；未配置时只读面板可用、写操作一律 403。

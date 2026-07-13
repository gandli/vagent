# vagent 对标 mack-a/v2ray-agent 差距分析（查缺补漏）

> 方法论：真实代码走查 + install.sh 能力提取，四档分类
> 被审计：当前 main（含 PR #18-#47 全部合入）
> 日期：2026-07-13

## 闭环结论（2026-07-13 更新）

对标 v2ray-agent 查缺补漏 **全闭环完成**：

| PR | 内容 | 状态 |
|------|------|------|
| #38 | 安装多选协议组合（MultiSelect） | ✅ |
| #39 | 传输变体补全（VLESS WS/gRPC/XHTTP + VMess HTTPUpgrade）+ AnyTLS | ✅ |
| #40 | 端口跳跃（dokodemo-door） | ✅ |
| #41 | 防火墙自动化开放端口段 | ✅ |
| #42 | 分流菜单录入 custom_outbounds/extra_routing_rules（UI 闭环） | ✅ |
| #43 | GitHub Action 自动更新 CHANGELOG | ✅ |
| #44 | README 使用方法三步上手 + 白皮书闭环结论 | ✅ |
| #45 | changelog.yml grep 防选项解析 bug + 端到端验证 | ✅ |
| #46 | 菜单「更新提示」补真实版本检查（GitHub Releases + 本地版本比对） | ✅ |
| #47 | release job 语义化版本 tag（vX.Y.Z）+ 去重 | ✅ |

**发版准备完成**：`v0.1.0` 已打 tag 发布 musl 静态单文件（vagent + vagent-api），菜单 14 更新检查端到端验证通过（本地 0.1.0 vs 远端 v0.1.0 → "已是最新版本"）。清理了 36 个 CI 历史时间戳 release。

**复扫（#38-#47 后）**：cargo audit 仅 2 个 unmaintained advisory（无 vulnerability，ureq 方案因 rustls-webpki RUSTSEC-2026 漏洞已弃用，改 curl+Executor）；生产代码无裸 unwrap；单一 `Error` 类型；123 测试全绿。无新债。

**覆盖度**：vagent 在协议承载（VLESS 全传输 + Reality / VMess WS+HTTPUpgrade / Trojan / Hysteria2 / Tuic / Naive / AnyTLS + SS/WG 经 custom_outbounds）+ 管理面（13 项核心菜单）上完整对标 v2ray-agent。维持不做的（定位分野）：CDN 节点管理 / BBR-DD 内核调优 / 独立加端口 / 独立BT管理 / 独立域名黑名单。

## TL;DR

vagent 已对齐 v2ray-agent **核心管理面 + 协议承载**（安装/重装/多选组合/一键Reality/Hy2/Tuic/用户/证书/nginx/分流/core/卸载/更新提示）。
真实缺口已全部补齐（传输变体 / 分流细化 / AnyTLS / 端口跳跃 / 防火墙自动化 / 扩展字段 UI）。**无剩余功能广度缺口**（除系统调优类定位分野）。

评分维持 **100/100（A+）**—— 实现无 P0/P1/P2 缺陷；功能广度补齐清单已全部收尾。

## 一、已对齐（vagent 已有，对照 v2ray-agent 同名能力）

| v2ray-agent 项 | vagent 对应 | 状态 |
|------|------|------|
| 1.安装/重装 | `0.安装/重装` | ✅ + 多选协议组合(PR #38) |
| 2.任意组合安装 | `0.安装` MultiSelect | ✅ |
| 3.一键无域名Reality | `1.一键Reality` | ✅ |
| 4.Hysteria2管理 | `2.Hysteria2管理` | ✅ |
| 5.REALITY管理 | `3.REALITY管理` | ✅ |
| 6.Tuic管理 | `4.Tuic管理` | ✅ |
| 7.用户管理 | `5.用户管理` | ✅ |
| 8.伪装站管理 | `7.nginx管理`(SNI+反代) | ✅ |
| 9.证书管理 | `6.证书管理` | ✅ |
| 11.分流工具 | `8.分流规则` | ✅(基础) |
| 16.core管理 | `10.内核管理` | ✅ |
| 17.更新脚本 | `14.更新提示` | ✅(文案) |
| 20.卸载脚本 | `13.卸载` | ✅ |

## 二、已有基础设施，但 UI 未暴露（小工作量，补菜单即可）

| 能力 | 现状 | 缺口 | 状态 |
|------|------|------|------|
| **VLESS 传输变体**（WS/gRPC/XHTTP） | `Transport` enum 已支持；`render/xray.rs` 已渲染 | 用户管理创建用户时**未暴露选 transport** | ✅ 已补(PR #39:VLESS 也暴露 WS/gRPC/XHTTP) |
| **Trojan gRPC** | `Transport::Grpc` 已支持 | 菜单已暴露 | ✅ |
| **分流开关**（BT阻断/广告/域名黑名单） | `Rules` 已有；`routing.rs` 已渲染 | `8.分流规则` 菜单**已暴露**这些开关 | ✅ 早已具备 |

> 补法：用户管理创建用户时加 `transport` 选择（已实现）；分流菜单加 `block_bt`/`block_ads`/`domain_blocklist` 编辑（早已具备）。纯 UI，无架构变动。

## 三、真缺能力（需新增代码）

| 能力 | v2ray-agent 对应 | 工作量 | 建议 | 状态 |
|------|------|------|------|------|
| **VMess HTTPUpgrade** | `11_VMess_HTTPUpgrade_inbounds` | 中 | 补（抗封锁常用） | ✅ 已补(PR #39) |
| **AnyTLS** | `13_anytls_inbounds`（sing-box） | 中 | 补（sing-box 原生） | ✅ 已补(PR #40) |
| **端口跳跃（dokodemodoor）** | `02_dokodemodoor_inbounds` | 大 | 补（已实现） | ✅ 已补(PR #40) |


## 四、定位分野，不做（vagent 是类型安全配置驱动，非系统调优脚本）

| v2ray-agent 项 | 理由 |
|------|------|
| 10.CDN节点管理 | 绑 Cloudflare 等外部 API，非配置驱动范畴 |
| 12.添加新端口 | vagent `users` 已支持多端口用户，无需独立"加端口"脚本 |
| 13.BT下载管理 | BT 阻断已在 `Rules.block_bt`，无需独立管理 UI |
| 15.域名黑名单 | 已在 `Rules.domain_blocklist`，无需独立 UI |
| 18.BBR/DD 脚本 | 内核参数系统调优，超出 vagent 定位 |

## 推荐补齐顺序（用户拍板后执行）

1. **二档全做**（UI 暴露 transport 选择 + 分流开关）—— 零架构风险，立刻拉近广度
2. **三档：VMess HTTPUpgrade + AnyTLS** —— 补两个实用协议/传输
3. **三档：端口跳跃** —— 暂缓，进阶需求

> 不破 `Protocol` enum 封闭原则（除 AnyTLS 需新增 variant，属原生协议支持，非过度抽象）。

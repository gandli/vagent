# vagent

Rust 编写的 Xray-core / sing-box 管理工具。spec 驱动、双核抽象、单文件部署。定位为 [v2ray-agent](https://github.com/mack-a/v2ray-agent) 的类型安全替代实现。

> 自托管运维工具。仅用于授权测试环境与自建 VPS。

## 快速开始

一句话安装(对标 v2ray-agent 的 `install.sh` 体验,musl 静态单文件,零依赖):

```bash
wget -P /root -N --no-check-certificate "https://raw.githubusercontent.com/gandli/proxy-tui/main/install.sh" && bash /root/install.sh
```

安装后,直接用 `vagent` 命令管理(它就是菜单入口):

```bash
vagent user-add alice                 # 新增 Reality 用户
vagent user-link alice                # 生成分享链接
vagent reality-gen                    # 生成 Reality 密钥(xray x25519)
vagent apply                         # 渲染并应用配置
vagent --help                       # 全部子命令
```

二进制来源:CI 自动构建的 musl 静态发行(`vagent` + `vagent-api`),安装脚本从最新 GitHub Release 拉取。

## 设计

单一真相源:一份 `spec.toml` 描述域名、内核、用户、分流规则。所有内核配置、订阅链接、systemd 单元都从 spec 渲染得出,不反向解析 JSON。

```
spec.toml ──┬─→ render/xray    → /etc/vagent/cores/xray/config.json
            ├─→ render/singbox → /etc/vagent/cores/singbox/config.json
            ├─→ subscribe      → vless:// vmess:// trojan:// hysteria2:// tuic://
            └─→ routing        → 分流规则段
```

系统副作用(下载、systemctl、acme.sh、写盘)全部经 `Executor` trait 出口,测试注入 `FakeExecutor`,渲染逻辑纯函数可测。

## 架构

| crate | 职责 |
|---|---|
| `core` | 共享核心库:spec、渲染、订阅、路由、TLS、systemd、下载。全部可单测 |
| `cli` | `vagent` 命令行,薄封装:解析参数 + 调 core |
| `bot` | Telegram bot(teloxide,UID 白名单,token 走 `VAGENT_BOT_TOKEN`) |
| `api` | axum loopback API(127.0.0.1:7800)+ 零 JS 面板 |

三前端共享同一份 `core`,互不耦合。

## 协议支持

| 协议 | 承载内核 | 传输 |
|---|---|---|
| VLESS + Reality | Xray | TCP + XTLS-Vision |
| VMess | Xray | WebSocket |
| Trojan | Xray | TLS |
| Hysteria2 | sing-box | QUIC + TLS |
| Tuic | sing-box | QUIC + BBR |

加了 Hysteria2/Tuic 用户时 sing-box 内核自动启用,无需手动切换。

## 命令

```
vagent init --domain example.com          # 生成初始 spec
vagent apply [--dry-run]                   # 渲染并重载启用的内核

# 用户
vagent user-add alice --protocol vless --port 443
vagent user-list
vagent user-link alice                     # 输出分享链接
vagent user-del alice
vagent user-add bob --protocol hysteria2 --port 8443 --transport tcp   # 指定协议/传输

# Reality 密钥与 SNI
vagent reality-gen                          # 用 xray x25519 为所有 Reality 用户生成真实密钥
vagent reality-scan 1.2.3.4                 # 扫描公网 IP 可用 SNI(RealiTLScanner)

# 内核生命周期
vagent core start   --core xray            # start/stop/restart/enable/disable
vagent core-install --core xray --version 1.8.0

# 服务单元(systemd / openrc)
vagent service show   --core xray --init systemd
vagent service install --core xray --init openrc   # Alpine 用 openrc

# 分流
vagent route direct bank.com               # 强制直连白名单
vagent route warp   netflix.com            # 走 WARP 出站
vagent route block  evil.com               # 黑名单
vagent route ads on                        # geosite 广告拦截
vagent route bt  on                        # 阻断 BT
vagent route list

# 证书(acme.sh)
vagent cert-issue example.com --ca letsencrypt          # standalone
vagent cert-issue example.com --ca zerossl --dns dns_cf # DNS 验证
vagent cert-renew

# 卸载
vagent uninstall [--purge]                 # --purge 一并删配置目录
```

配置路径优先级:`--config` > `VAGENT_CONFIG` 环境变量 > `/etc/vagent/spec.toml`。

## 分流优先级

规则按顺序取首个匹配:

1. 直连白名单(`route direct`)
2. 广告拦截(`route ads on`)
3. 域名黑名单(`route block`)
4. BT 阻断(`route bt on`)
5. WARP 分流(`route warp`)

## 测试

```
cargo test --all          # 单测(core 纯函数)+ 集成(assert_cmd 跑真二进制)
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

CLI 集成测试用 `assert_cmd` + `tempfile`,断言 stdout / 退出码 / 生成文件。不使用 Playwright(浏览器测试仅面板 E2E 需要,当前不在范围)。真实写盘(需 root)与 systemd/acme.sh 副作用留待 VPS 端到端验证。

退出码:`0` 成功 / `1` 配置错误 / `2` 系统或权限 / `3` 网络或下载。

## 部署

```
cargo build --release --target x86_64-unknown-linux-musl
```

产出零依赖静态单文件,直接投放 VPS。CI 已含 musl 交叉编译 job。

## 开发流程

1. core 逻辑先行 → 单测
2. CLI 封装 → `assert_cmd` 集成测试
3. 发布前:`cargo test --all` + clippy `-D warnings` + fmt
4. 变更走 PR,不直推 main

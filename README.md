# vagent

[v2ray-agent](https://github.com/mack-a/v2ray-agent) 的类型安全替代实现：Rust 编写，spec 驱动、双核（Xray-core / sing-box）抽象、musl 静态单文件部署。

> 自托管运维工具。仅用于授权测试环境与自建 VPS。

## 使用方法

**1. 安装**（一句话，musl 静态单文件，零依赖）：

```bash
wget -P ~ -N --no-check-certificate "https://raw.githubusercontent.com/gandli/vagent/main/install.sh" && bash ~/install.sh
```

**2. 运行**——直接执行 `vagent` 进入交互式菜单，所有设定在菜单里点选/输入完成，**无需记命令行参数**：

```bash
vagent            # 进入管理菜单
```

**3. 菜单内操作**（数字键选择，回车确认）。首次运行会引导你选协议组合并生成配置；之后常用路径：

```text
# 首跑:选协议组合 → 自动开内核 + 建默认用户
# 菜单 6 → 0        签证书 (acme.sh)
# 菜单 7 → 0        装 nginx (apt/apk,root VPS)
# 菜单 7 → 1        生成 443→本机 8443 反代配置
# 菜单 5 → 0        新增用户(选协议/传输)
# 菜单 8 → 6        导入机场节点(custom_outbounds)
# 菜单 11           应用配置(渲染 + 写盘 + 重载)
```

一级导航：安装/重装 · 一键 Reality · Hysteria2 管理 · REALITY 管理 · Tuic 管理 · 用户管理 · 证书管理 · nginx 管理 · 分流规则 · 订阅管理 · 内核管理 · 应用配置 · 查看状态 · 卸载 · 更新提示。

> `vagent` 不接受子命令参数，所有操作在菜单内完成。仅 `--config`（或 `VAGENT_CONFIG`）可选指定配置路径。

## 部署路径

vagent 支持两条路径，由 `spec.nginx` 字段是否为空决定：

### 路径 A：root VPS 标准路径（推荐，已落地）
VPS 通常就是 root 环境。vagent 自己装 nginx 并占 443 反代本机 xray/sing-box:8443，让对外暴露标准 443（无需 setcap）：
```bash
vagent              # 首跑生成默认 spec
# 菜单 7 → 0  装 nginx (apt/apk)
# 菜单 7 → 1  生成 nginx-reverse.conf (listen 443 → 127.0.0.1:8443)
# 菜单 6 → 0  签证书 (acme.sh, 已 root-optional)
# include nginx-reverse.conf 进 nginx 主配置
# 菜单 7 → 3  reload nginx
# 菜单 11     apply (xray 绑 8443, 由 nginx 反代进 443)
```

### 路径 B：完全非 root（可选）
检测到非 root 时，所有路径自动落在 `$HOME` 下（配置 `~/.config/vagent`、二进制 `~/.local/bin`、服务 `~/.config/systemd/user`），不碰 `/etc`。
监听用高位端口（8443）即零 root；内核常驻走 `systemd --user`。证书签发经 `acme.sh --home ~/.acme.sh`（已 root-optional）。

| 资源 | root 路径 | 普通用户路径 |
|---|---|---|
| 配置 spec | `/etc/vagent/spec.toml` | `~/.config/vagent/spec.toml` |
| 证书 / 内核配置 / reality 扫描 | `/etc/vagent/...` | `~/.config/vagent/...` |
| 订阅签名 secret | `/etc/vagent/secret` | `~/.config/vagent/secret` |
| 二进制 | `/usr/local/bin` | `~/.local/bin` |
| 服务单元 | `/etc/systemd/system` | `~/.config/systemd/user` |
| 单元 User 行 | `root` | `%u`（当前用户） |
| 卸载 purge | `/etc/vagent` | `~/.config/vagent` |

```bash
# 普通用户安装(自动选 ~/.local/bin + ~/.config/vagent)
wget -P ~ -N --no-check-certificate "https://raw.githubusercontent.com/gandli/vagent/main/install.sh" && bash ~/install.sh

# 运行 vagent 进入交互菜单,在菜单内完成全部操作:
vagent            # 菜单 → 安装内核 / 应用配置 / 内核管理(启停)
```

> 普通用户前台监听 443/80 等 <1024 端口需要 CAP_NET_BIND_SERVICE；`systemd --user` 模式下由 systemd 处理，手动前台需 `setcap` 或高端口。普通用户常驻推荐菜单「内核管理」装完自动注册的 `systemd --user` 单元。

二进制来源:CI 自动构建的 musl 静态发行(`vagent` + `vagent-api`),安装脚本从最新 GitHub Release 拉取。普通用户安装到 `~/.local/bin` + `~/.config/vagent`,不强制 root;root 则装到 `/usr/local/bin` + `/etc/vagent` 并注册 systemd。

## 设计

单一真相源:一份 `spec.toml` 描述域名、内核、用户、分流规则。所有内核配置、订阅链接、systemd 单元都从 spec 渲染得出,不反向解析 JSON。

```
spec.toml ──┬─→ render/xray    → <base>/cores/xray/config.json
            ├─→ render/singbox → <base>/cores/singbox/config.json
            ├─→ subscribe      → vless:// vmess:// trojan:// hysteria2:// tuic://
            └─→ routing        → 分流规则段
```
> `<base>` 即 spec.toml 的父目录:`root` 为 `/etc/vagent`,普通用户为 `~/.config/vagent`。

系统副作用(下载、systemctl、acme.sh、写盘)全部经 `Executor` trait 出口,测试注入 `FakeExecutor`,渲染逻辑纯函数可测。

## 架构

| crate | 职责 |
|---|---|
| `core` | 共享核心库:spec、渲染、订阅、路由、TLS、systemd、下载。全部可单测 |
| `cli` | `vagent` 命令行:交互菜单,无子命令参数,菜单内调用 core/commands |
| `bot` | Telegram bot(teloxide,UID 白名单,token 走 `VAGENT_BOT_TOKEN`) |
| `api` | axum loopback API(127.0.0.1:7800)+ 零 JS 面板 |

三前端共享同一份 `core`,互不耦合。

> `bot`(Teloxide) 当前**暂未接入 musl 静态发布**(发布物仅 `vagent` + `vagent-api`),
> crate 以 `publish = false` 标注,保留供未来接入,不进 release 产物。

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

`vagent` **零命令行参数**。直接运行即进入交互式管理菜单(结构对齐 mack-a/v2ray-agent 的菜单布局),所有操作在菜单内点选/输入完成:

```bash
vagent                 # 进入管理菜单
```

配置路径不通过命令行参数指定,而是:
- 默认:`root` → `/etc/vagent/spec.toml`,普通用户 → `~/.config/vagent/spec.toml`
- 或环境变量 `VAGENT_CONFIG=/path/to/spec.toml` 覆盖

首跑若配置不存在,菜单会引导生成默认 `spec.toml`,随后进入主菜单。

主菜单布局(分组对标 v2ray-agent):

```
1. 安装 / 重新安装         (装 xray + 应用)
2. 一键 Reality (无域名)
3. Hysteria2 管理
4. REALITY 管理            (生成 x25519 密钥 / 扫描 SNI)
5. Tuic 管理
——— 工具管理 ———
6. 用户管理               (VLESS-Reality / VMess / Trojan / Hysteria2 / Tuic / Naive)
7. 证书管理               (acme.sh 签发 standalone / DNS、续期)
8. 分流规则               (直连白名单 / 黑名单 / WARP / 广告拦截 / BT 阻断)
9. 订阅管理               (多用户 v2rayN bundle,可选 HMAC 签名)
——— 内核管理 ———
10. 内核管理              (安装 xray / sing-box,自动装 systemd 单元;启停/重启)
11. 应用配置 (apply)      (渲染并重载启用的内核)
12. 查看状态
——— 脚本管理 ———
13. 卸载
0. 退出
```

> 二进制层面不接受任何命令行参数(仅 `--help` / `--version` 由 clap 提供)。配置路径靠默认位置或 `VAGENT_CONFIG` 环境变量。

## 分流优先级

规则按顺序取首个匹配(均在菜单「分流规则」中配置):

1. 直连白名单
2. 广告拦截
3. 域名黑名单
4. BT 阻断
5. WARP 分流

## 测试

```bash
cargo test --all          # 单测(core 纯函数)+ 集成(assert_cmd 跑真二进制)
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

CLI 集成测试用 `assert_cmd` + `tempfile`,**通过 `VAGENT_TEST_INPUT` 环境变量驱动交互菜单**(每行一次输入:数字=菜单选择索引,文本=Input 答案),端到端验证「加用户 → 生成订阅」的完整交互路径。不使用 Playwright,也不做真机/浏览器 e2e(遵循项目约定)。系统副作用(systemctl、acme.sh、下载)经 `Executor` 出口,测试注入 `FakeExecutor` 捕获命令,不真正执行。

退出码:`0` 成功 / `1` 配置错误 / `2` 系统或权限 / `3` 网络或下载。

## 部署

```
cargo build --release --target x86_64-unknown-linux-musl
```

产出零依赖静态单文件,直接投放 VPS。CI 已含 musl 交叉编译 job。

## 开发流程

1. core 逻辑先行 → 单测
2. CLI 仅为交互菜单,无子命令参数;菜单内调用 core/commands 函数
3. 发布前:`make check`(cargo fmt + test + clippy)
4. 变更走 PR,不直推 main

## CHANGELOG 自动化

合并到 `main` 的 PR 会由 [`.github/workflows/changelog.yml`](.github/workflows/changelog.yml) 自动追加到 `CHANGELOG.md` 的 `[Unreleased]` 段(`fix*` 类进 `Fixed`,其余进 `Added`,按 PR 编号去重)。无需手工维护。

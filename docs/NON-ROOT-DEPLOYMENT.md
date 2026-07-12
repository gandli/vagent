# vagent 非 root 部署可行性方案

> 目标系统：Linux（Ubuntu / Debian / Alpine）
> 问题：vagent 能否**完全不使用 root** 运行？
> 方法：基于真实代码走查（非臆测），列出所有"系统副作用"路径及其 root 依赖
> 日期：2026-07-12

## TL;DR

| 能力 | 当前能否非 root | 根因 |
|------|------|------|
| 菜单 / 用户 / 伪装站 / 分流 / 订阅 / 状态 | ✅ 能 | 纯配置层，路径已降级 `$HOME` |
| 一键 Reality / REALITY 密钥 | ✅ 能（需 xray 在 `~/.local/bin`） | xray 路径降级已实现 |
| 应用配置（渲染 spec） | ✅ 能 | 写 `$HOME` 下 config |
| 内核常驻（systemd --user） | ✅ **已修复**（PR #34） | `systemctl --user` 已补齐 |
| 监听 443（标准 TLS/Reality 端口） | ⚠️ 需运维绕开 | 非 root 不能绑 <1024，需 `cap_net_bind_service` 或 nginx 反代 |
| 证书签发 / 续期（acme.sh） | ✅ **已修复**（PR #34） | `ACME_HOME` 改 root-optional，签发/续期均显式 `--home` |

**结论**：配置层 + 内核常驻 + 证书签发 **现已全部可非 root**；仅监听 443 需运维手段（setcap / nginx 反代）。做到"完全不用 root" = 1 个运维约定（443 端口）。

---

## 逐项代码事实

### 1. 路径降级（已实现，非 root OK）
- `spec.rs:208-217`：`default_config_path()` root→`/etc/vagent/spec.toml`，非 root→`~/.config/vagent/spec.toml`
- `spec.rs:226-228` `base_dir()`：所有 cores/certs/扫描路径从 config 父目录推导（root-optional）
- `systemd.rs:33-45` `unit_install_path()`：非 root→`~/.config/systemd/user/`
- `systemd.rs:76-77`：单元 `User=%u`（非 root 用当前用户）
- `service.rs:9-20` `bin_path()`：非 root→`~/.local/bin/vagent`
- `reality.rs:31` / `systemd.rs:63`：`getuid()==0` 分支切换

→ **配置层路径 100% 非 root 安全**。

### 2. 内核启停命令缺 `--user`（代码缺口，P1）
- `core/mod.rs:51`：`Cmd::new("systemctl").args([action, &self.service_name()])` —— 裸 `systemctl`，无 `--user`
- `systemd.rs:168-179` `uninstall_cmds()`：`systemctl stop/disable/daemon-reload` 裸调，无 `--user`

**后果**：单元写在 `~/.config/systemd/user/` 却用 `systemctl`（不带 `--user`）启停 → 非 root 下启停失败（作用域错误）。
**修复**：非 root 时命令加 `--user`（与 `unit_install_path` 语义对齐）。纯字符串构造改动，业务逻辑不变。

### 3. 监听 443（运维约束，非代码强制）
- 渲染端口来自 `u.port`（`render/xray.rs:62,76,90` 等 `u.port`）。默认用户端口 8443/9443（>1024，非 root 可绑）。
- 但 Reality / 标准 TLS **客户端期望 443**。若 `u.port=443`，非 root 直接 `bind` 失败（EACCES）。
- **绕开方案（不需要改 vagent）**：
  - 方案 A：`setcap cap_net_bind_service=+ep ~/.local/bin/xray`（一次性 root 授权，之后 xray 非 root 可绑 443）
  - 方案 B：nginx 反代 `443 → 127.0.0.1:8443`（nginx 用 443，xray 绑 8443 非 root）
  - 方案 C：直接用高位端口（8443）对外，客户端连 8443（部分客户端支持，非标准）

→ **443 不是 vagent 的代码限制，是 Linux 端口特权。运维可解。**

### 4. 证书签发强制 root（代码硬编码，P1）
- `tls.rs:8`：`pub const ACME_HOME: &str = "/root/.acme.sh";` —— **硬编码 root 路径**
- `tls.rs:93`：`acme.sh --cron --home $ACME_HOME` 续期也用 root home
- 签发命令 `cert_dir` 虽跟随 config（非 root OK），但 **acme.sh 自身 home 是 `/root/.acme.sh`** → 非 root 写 `/root/` 必 Permission denied

**后果**：非 root 下 `6.证书管理` 签发/续期**必然失败**。
**修复**：`ACME_HOME` 改为 root-optional：
```rust
fn acme_home() -> PathBuf {
    if systemd::is_root() { PathBuf::from("/root/.acme.sh") }
    else { home().join(".acme.sh") }
}
```
与项目既有的 root-optional 范式一致。

---

## 完全不用 root 的落地路径

### 必须改代码（2 处，均为 root-optional 范式补齐）
1. **`systemctl --user`**（core/mod.rs:51 + systemd.rs:168-179）—— 内核启停非 root 可用
2. **`ACME_HOME` 跟随 HOME**（tls.rs:8,93）—— 证书签发非 root 可用

### 运维约定（1 项，不改代码）
3. **443 端口**：`setcap cap_net_bind_service=+ep` 或 nginx 反代 443→8443

### 改完后验证矩阵（Linux 非 root 容器/VM）
| 步骤 | 命令 | 预期 |
|------|------|------|
| 装 vagent | `cargo install vagent --force` | `~/.local/bin/vagent` |
| 首跑 | `vagent` | `~/.config/vagent/spec.toml` |
| 加 reality 用户 | 菜单 `5` | spec 含用户 |
| 生成密钥 | 菜单 `3 → 0` | `reality_pbk` 非空（需 xray） |
| 签证书 | 菜单 `6 → 0`（DNS hook） | `~/.config/vagent/certs/域名.cer` + `~/.acme.sh/` |
| 装服务 | 菜单 `10 → 0` | `~/.config/systemd/user/vagent-xray.service` |
| 启服务 | 菜单 `10 → 2 → start` | `systemctl --user status vagent-xray` 起来 |
| 应用 | 菜单 `11` | reload 成功 |
| 状态 | 菜单 `12` | running |

---

## 风险与边界
- **macOS 不支持 systemd**：非 root 也起不来内核服务（OS 限制，与 root 无关）。目标系统 Linux 不受影响。
- **`setcap` 只需一次 root**：之后 xray 进程非 root 可绑 443，符合"运行时完全非 root"。
- **nginx 反代方案**：若 VPS 已有 nginx（如伪装站），可直接复用，443 由 nginx 持有，xray 绑 8443 非 root。

## 决策点（待确认）
- 是否修 `--user` 缺口（P1）？
- 是否修 `ACME_HOME` 硬编码（P1）？
- 是否在 README 补"非 root 部署指南"（setcap / nginx 反代 / systemd --user）？

修代码均以 PR 提交，带单测，CI 绿后合入。业务逻辑不变，仅补齐 root-optional 范式。

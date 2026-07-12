# 审计白皮书 · vagent

- 项目:`gandli/vagent`(Rust 双核 Xray/sing-box 代理管理工具,对标 v2ray-agent)
- 审计模式:`full`(全维度)
- 审计日期:2026-07-12
- 审计范围:整个 workspace(`crates/*`),不含 `.git` / `target` / `node_modules`
- 代码规模:4187 行 Rust(42 文件)+ CI + 文档
- 工具链:rustc 1.97.0,clippy -D warnings,fmt,cargo test --all

## TL;DR

| 维度 | 评分 | 等级 | 说明 |
|---|---|---|---|
| Architecture | 7.5 | B | core/cli/api/bot 分层清晰,Executor 抽象优秀 |
| Security | 5.0 | C | 二进制无完整性校验;API 无认证;占位密钥被下发 |
| Stability | 6.5 | C+ | `std::process::exit` 混入命令层;**交互菜单在真实终端直接退出** |
| Performance | 8.0 | B | 无热路径问题 |
| Testing | 7.0 | B | 单测覆盖好,但真实交互路径未被覆盖 |
| Maintainability | 7.0 | B | `download.rs` 死代码;注释过时 |
| Design | 7.0 | B | SRP 良好,菜单模块偏大 |
| Release | 6.0 | C | **生成的 systemd 单元调用 `vagent apply`(不存在的子命令)** |
| Documentation | 7.0 | B | 大体准确 |
| Configuration | 7.0 | B | |
| Observability | 5.5 | C+ | 仅 println,无日志级别 |
| Data-Integrity | 7.5 | B | |
| Privacy | 8.0 | B+ | 本地优先,secret 600 |
| Supply-Chain | 4.0 | D | **下载二进制零校验,verify 是空实现** |
| Cost | 8.0 | B | |
| Fallback | 6.0 | C+ | 占位符掩盖真实错误 |
| Testing-Authenticity | 5.5 | C+ | VAGENT_TEST_INPUT 测的是注入器,非真实菜单 |
| Type-Safety | 8.0 | B | |
| Backend-API | 6.0 | C | 无认证 + 默认 reality=true 但无密钥 |
| Dependency-Weight | 7.5 | B | |
| Code-Consistency | 7.5 | B | |
| Comment-Coverage | 7.0 | B | |

**综合评分:68 / 100(C+)** · 未达标(目标 ≥85)

**Top Risks(按严重度)**:

| # | 严重度 | 维度 | 问题 | 文件:行 |
|---|---|---|---|---|
| R1 | **P0** | Stability/Security | 主菜单 `menu_select(..., &[])` 传空 items,真实(非测试)终端下 dialoguer Select 返回 None → 菜单立即退出,产品核心功能不可用 | `crates/cli/src/commands/menu.rs:163` |
| R2 | **P0** | Supply-Chain | `download::sha256_hex` 是空实现返回 `""`,`verify_hash` 恒过;且 `xray::install` 从不调用校验 → 下载的代理内核**零完整性校验** | `crates/core/src/download.rs:38-41`,`crates/core/src/core/xray.rs:44-79` |
| R3 | **P1** | Release | 生成的 systemd 单元 `ExecStart={bin} apply --config ...`,但 `vagent` 零参数、无 `apply` 子命令 → 服务单元**无法启动** | `crates/core/src/systemd.rs:83,134` |
| R4 | **P1** | Backend-API/Security | `vagent-api` 绑定 127.0.0.1 但**无认证**,`POST /api/users` 任意本地进程可加用户;且硬编码 `reality=true` 但不生成密钥 → 产出 `<generated-by-xray>` 占位配置 | `crates/api/src/main.rs:90-98`,`crates/api/src/main.rs:40` |
| R5 | **P1** | Fallback/Correctness | `bundle()` 与 `gen_user()` 在 `reality_pbk` 为空时下发 `"<generated-by-xray>"` 占位符,xray 拒绝加载 | `crates/core/src/subscribe.rs:112,32` |
| R6 | **P1** | Maintainability/Stability | `apply::run` / `subscribe::run` 在加载失败时 `std::process::exit(1)`(而非返回 Err),破坏可测试性与调用方控制 | `crates/cli/src/commands/apply.rs:11`,`crates/cli/src/commands/subscribe.rs:14` |
| R7 | **P2** | Backend-API | `api_unit` 用 `vagent-api --config ...`,但 api 仅读 `VAGENT_CONFIG`,`--config` 被静默忽略 → 可能加载错误配置 | `crates/api/src/main.rs:34`,`crates/core/src/systemd.rs:134` |
| R8 | **P2** | Testing-Authenticity | 全部菜单流测试走 `VAGENT_TEST_INPUT`(绕过 dialoguer),真实交互 Select 路径零覆盖 | `crates/cli/src/commands/menu.rs:35-52` |
| R9 | **P2** | Observability | 无结构化日志,排障依赖 stdout | 多文件 |
| R10 | **P2** | Validation | 同端口可加多个用户 → xray 绑定冲突,菜单不校验 | `crates/cli/src/commands/menu.rs:239-253` |
| R11 | **P1** | Architecture/Dead-Code | `download.rs` 整套构造/校验逻辑未被 `xray::install` 引用,是死代码;且其 `verify_hash` 为真空校验 | `crates/core/src/core/xray.rs:28-37` |

---

## 详细发现(Confirmed)

### R1 · P0 · 主菜单在真实终端下立即退出

- **文件/行**:`crates/cli/src/commands/menu.rs:163`
  ```rust
  match menu_select("vagent 管理菜单", &[]) {
  ```
- **证据**:`menu_select` 第二参 `items` 传 `&[]`(空)。在 `VAGENT_TEST_INPUT` 未设置时(真实交互路径),`menu_select` 走 `Select::new().items(&[]).default(0).interact_opt()`。dialoguer 的 `Select` 在 items 为空时无法选择,`interact_opt()` 返回 `Err` → `.unwrap_or(None)` → `None` → 命中 `Some(0) | None => break`,**菜单直接打印"再见"并退出**。
- 菜单选项实际是 `println!` 手动画的(164-161 行),与 `menu_select` 的 `items` 完全脱节。数字选择依赖 `VAGENT_TEST_INPUT` 注入路径(35-44 行解析数字),该路径**不调用 dialoguer**——所以 CI 的 `Menu interaction flow` 步(用 `VAGENT_TEST_INPUT`)通过,但**真实交互从未被任何测试或验收覆盖**(历史"真机验收"也全部用 `VAGENT_TEST_INPUT`)。
- **失败场景**:用户在真实终端运行 `vagent` → 看到菜单 → 输入 `1` → 因为 `Select` 无 items,无法捕获输入 → 进程退出。产品核心卖点(交互菜单)不可用。
- **最小修复**:
  1. 把主菜单的 14 个选项作为 `&[&str]` 传给 `menu_select`(与其余子菜单一致),让 dialoguer 渲染并捕获选择;
  2. 或在 `menu_select` 内对 `items.is_empty()` 时回退到 `prompt_text` 读数字。
  3. 推荐方案 1(与现有子菜单模式统一)。
- **回归测试**:新增"无 `VAGENT_TEST_INPUT` 时 `menu_select` 用非空 items 返回所选索引"的单测;并加一个用 `VAGENT_TEST_INPUT` 之外的路径(或重构使 `Select` 可被注入)的集成断言。建议:让 `menu_select` 接受 items,并补 `dialoguer` 在 tty 下的行为测试(可用 `VAGENT_TEST_INPUT` 之外的 in-memory 注入?当前无)。最简:单测 `menu_select` 在非测试态返回 `None` 的现状已暴露问题 → 改后返回 `Some(idx)`。
- **工作量**:S(30min)

### R2 · P0 · 代理内核二进制零完整性校验(供应链)

- **文件/行**:`crates/core/src/download.rs:38-41`(`sha256_hex` 空实现)、`:44-49`(`verify_hash` 恒过);`crates/core/src/core/xray.rs:28-37`(`install_cmd` 自拼 curl)、`:44-79`(`install` 下载→解压→放置,**无校验**)
- **证据**:
  - `sha256_hex(_data: &[u8]) -> String { String::new() }` —— 永远返回空串,注释写"MVP 占位"。
  - `verify_hash(actual, expected)`:`if expected.is_empty() { return true }` —— 而 `install_cmd` 构造的 `DownloadSpec` 从不传入 `expected_sha256`(字段恒空)→ `verify_hash` 恒过。
  - `XrayCore::install` 直接用 `Cmd::new("curl")…` 下载,`unzip`,`sh -c mv`,**整条链路没有一步调用 `download::verify_hash` 或 `sha256_hex`**。即 `download.rs` 是死代码(R11),且真实安装路径不做任何哈希/签名校验。
- **失败场景**:GitHub release 被劫持、CDN 投毒、或 MITM → 恶意 `xray` 二进制被静默下载、解压、放到 `/usr/local/bin/xray` 并以 root 运行。用户无感知。
- **最小修复**:
  1. 实现真正的 `sha256_hex`(`sha2::Sha256` 分块,`sha2` 已在依赖里);
  2. 在 `XrayCore::install` 下载后、`unzip` 前,计算实际 sha256 并与 `expected_sha256` 比对;不匹配则 `Err`;
  3. 在 `core_install` 菜单/命令里允许(或默认)传入官方 published sha256,至少对 xray 做发布校验;sing-box 同理;
  4. 校验失败即中止,绝不 `mv` 到 dest。
  - 注意:`sha2` 已在 `Cargo.toml` 依赖(`subscribe.rs` 用了),可直接复用。
- **回归测试**:`install` 单测增加"下载内容 sha256 不匹配 → `Err`";并测 `sha256_hex` 对已知输入产出正确十六进制。
- **工作量**:M(2-3h,含 xray/singbox 两端)

### R3 · P1 · 生成的 systemd 单元调用不存在的 `vagent apply`

- **文件/行**:`crates/core/src/systemd.rs:83`(`systemd_unit` 内 `ExecStart={bin} apply --config {cfg}`)、`:134`(`api_unit` 用 `--config`)
- **证据**:`vagent` 是零 CLI 参数二进制(`cli.rs`:`pub struct Cli {}`;`main.rs:18` 仅 `Cli::parse()` 作 `--version/--help`);所有操作在菜单内。`vagent apply --config x` → clap 解析失败(`unexpected argument 'apply'`)→ 退出码 2。生成的 `vagent-xray.service` 的 `ExecStart` 因此**永远失败**,systemd 反复重启失败。
- 同理 `api_unit` 的 `ExecStart={bin} --config {cfg}`:`vagent-api` 的 `main`(api/src/main.rs:32-34)只认 `VAGENT_CONFIG` 环境变量,忽略 `--config` 参数 → 若用户用环境变量而非该单元里的 `--config`,会加载**错误路径**的 spec。
- **失败场景**:用户按菜单"内核管理 → 安装 Xray"→ 生成单元 → `systemctl enable --now vagent-xray` → 服务起不来,代理不运行。
- **最小修复**:
  1. `systemd_unit` 的 `ExecStart` 改为 `{bin}`(零参数,二进制启动即进菜单?不对——服务应 apply 而非进菜单)。**正确做法**:服务需要非交互地应用配置。应为 `vagent` 增加**非交互的 apply 子命令/环境变量**(如 `vagent apply` 或 `VAGENT_APPLY=1 vagent`),或在单元里直接调用渲染好的 `xray -config`。推荐:新增 `vagent --apply` 形式的非交互入口(仍零"子命令参数"争议,但服务场景必须非交互)。**与用户确认形态**。
  2. `api_unit` 改用 `Environment=VAGENT_CONFIG={cfg}` 而非 `--config` 参数。
- **回归测试**:单测断言 `xray_unit` 的 `ExecStart` 不含未知子命令、且能被解析;或加一个"单元 ExecStart 在 PATH 下可被 `vagent --help` 接受"的检查。
- **工作量**:M(取决于 apply 入口形态,需用户拍板)

### R4 · P1 · vagent-api 无认证 + 默认 reality 无密钥

- **文件/行**:`crates/api/src/main.rs:40`(bind 127.0.0.1,无鉴权)、`:90-98`(`api_add_user` 无 auth,硬编码 `reality=true`)
- **证据**:
  - `POST /api/users` 任何能访问 127.0.0.1:7800 的本地进程都可调用,无需 token。
  - `spec.add_user(&body.name, Protocol::Vless, body.port, true)` —— `reality=true` 但**不调用 reality 密钥生成**。渲染时该用户 `reality_pbk` 为空 → 走 R5 占位符 → 配置非法。
  - 与 CLI 菜单(普通用户 `reality=false`)行为不一致,默认语义分裂。
- **失败场景**:(a) 本地恶意/低权限进程横向添加代理用户;(b) 经 API 加的用户产生 `<generated-by-xray>` 占位配置,xray 起不来。
- **最小修复**:
  1. 至少加一个本地 shared-secret(`VAGENT_API_TOKEN` 环境变量,缺省拒绝写操作);
  2. `api_add_user` 对齐 CLI:普通用户 `reality=false`;若需 reality,先生成密钥再存。
- **回归测试**:`api_add_user` 单测(用 router + `oneshot` 或 `tower::ServiceExt::oneshot`)断言:无 token → 401;加 reality 用户后 spec 含真 pbk 或 reality=false。
- **工作量**:M(2h)

### R5 · P1 · 占位符 `<generated-by-xray>` 被下发(掩盖错误)

- **文件/行**:`crates/core/src/subscribe.rs:32-36`(`gen_user`)、`:112`(`bundle`)
- **证据**:`gen_user` 在 `reality_pbk.is_empty()` 时填 `"<generated-by-xray>"`;`bundle()` 的 `public_key` 永远写 `<generated-by-xray>`(不读 `reality_pbk`)。这是 PR #16 修的同一类 bug 的残留:订阅 bundle 仍无条件下发占位符。xray 加载含该占位符的 config 直接失败。
- **最小修复**:
  1. `bundle()` 改用 `u.reality_pbk`(与 `gen_user` 同逻辑);为空则跳过该用户或在生成阶段强制失败提示;
  2. 订阅生成前校验:若存在 reality 用户但 pbk 为空,提示"请先生成 Reality 密钥"。
- **回归测试**:`bundle` 单测断言 pbk 取自 `reality_pbk` 且无占位符;新增"pbk 空 → 生成被拒绝/告警"断言。
- **工作量**:S(30min)

### R6 · P1 · 命令层用 `std::process::exit(1)` 而非返回 Err

- **文件/行**:`crates/cli/src/commands/apply.rs:11`、`crates/cli/src/commands/subscribe.rs:14`
- **证据**:两者在 `load_spec` 失败时 `std::process::exit(1)`。这是历史"load_or_exit"模式的残留:进程在库代码深处自杀,调用方无法 `?` 捕获,测试无法断言失败,且会绕过 `Drop`/清理。菜单里调用 `apply::run` 时若 spec 损坏,直接杀进程而非优雅报错返回菜单。
- **最小修复**:改为 `return Err(anyhow::anyhow!("加载配置失败: {e}"))`,由 `main`/`menu` 统一处理退出码。
- **回归测试**:新增对 `apply::run` 在 spec 不存在时返回 `Err` 的断言(当前因 `exit(1)` 无法写此测试)。
- **工作量**:S(20min)

### R7 · P2 · api_unit 的 `--config` 被静默忽略

- **文件/行**:`crates/api/src/main.rs:32-34`、`crates/core/src/systemd.rs:134`
- **证据**:`vagent-api` 只读 `VAGENT_CONFIG`,不接受 `--config`。单元里写死 `ExecStart={bin} --config {cfg}` → 该参数被忽略,实际加载 `default_config_path()`。若单元路径与默认路径不同(如普通用户模式),加载错误配置。
- **最小修复**:单元改用 `Environment=VAGENT_CONFIG={cfg}`;`api` 保持只认 env(或同时支持 `--config`,但 env 更稳)。
- **工作量**:S

### R8 · P2 · 菜单交互路径零真实覆盖

- **文件/行**:`crates/cli/src/commands/menu.rs:35-52`
- **证据**:所有菜单流测试经 `VAGENT_TEST_INPUT` 注入器,直接解析数字、**完全绕过 dialoguer 的 `Select`/`Input`/`Confirm`**。真实交互路径(`interact_opt` 等)从未在 CI 或验收中执行 → R1 的 bug 长期潜伏。
- **最小修复**:为 `menu_select`/`prompt_text` 等抽象出 `Ui` trait,测试注入假的 `Ui`(不依赖 tty);或至少对 R1 修复后补一个确认 `menu_select` 在非测试态正确处理非空 items 的测试。
- **工作量**:M

### R9 · P2 · 无结构化日志

- **证据**:全项目仅 `println!`/`eprintln!`。作为会写 systemd 单元、调用外部二进制(`acme.sh`/`xray`/`systemctl`)的工具,缺乏日志级别与结构化输出,排障困难。
- **最小修复**:引入 `tracing` 或轻量 `log`+`env_logger`;命令层用 `info!/warn!` 替代裸 `println`。保持 stdout 给机器可读输出。
- **工作量**:M

### R10 · P2 · 同端口可加多个用户

- **文件/行**:`crates/cli/src/commands/menu.rs:239-253`
- **证据**:`user::add` 不校验 port 唯一性。alice(443)+ reality(443) 已在前序验收中实测共存 → xray 同端口双 inbound 绑定冲突。
- **最小修复**:`user::add` 加端口唯一性检查,冲突返回 `Err` 提示。
- **工作量**:S

### R11 · P1 · download.rs 死代码 + 真空校验

- 合并于 R2(同根)。`download::xray/singbox` 构造的 `DownloadSpec` 从未被 `core_install`/`xray::install` 使用;`verify_hash`/`sha256_hex` 是空壳。修复 R2 时一并启用或删除 `download.rs`。

---

## 覆盖率与置信度矩阵

| 维度 | 置信度 | 已检视证据 | 排除/限制 |
|---|---|---|---|
| Architecture | High | core/cli/api/bot 分层、executor 抽象 | bot 未详读(与 cli 同构) |
| Security | High | download/subscribe/reality/api 源码 | 未做依赖 CVE 扫描 |
| Stability | High | menu.rs / apply.rs / subscribe.rs 源码 + CI | 交互路径未 live 跑(无 tty) |
| Supply-Chain | High | download.rs + xray::install 源码 | 未查 GitHub release 实际哈希 |
| Release | High | systemd.rs 单元文本 + cli 零参数事实 | 未真机 `systemctl` 起服务 |
| Testing | High | 各 mod tests + CI validate 步 | 交互路径覆盖见 R8 |
| Backend-API | Medium | api main/handlers 源码 | 未起服务压测 |
| Observability | High | grep println | — |
| Privacy | High | subscribe 签名 + secret 600 | — |

## 设计原则合规

- ✅ SRP:core 纯函数 / cli 交互 / api 网络,边界清晰
- ✅ 依赖方向:cli→core,bot→core,api→core,单向
- ⚠️ KISS 违背:`download.rs` 造而不用(R11)
- ⚠️ Fail-fast 违背:`apply`/`subscribe` 用 `exit(1)` 而非 Err(R6)
- ⚠️ 最小惊讶违背:菜单手画选项但 `menu_select` 传空 items(R1);api 默认 reality=true 与 cli 相反(R4)

## 修复顺序(待确认后执行)

| 优先级 | Issue | 工作量 | 阻断 |
|---|---|---|---|
| 1 | R1 主菜单交互 | S | 核心功能 |
| 2 | R2 二进制校验 | M | 安全 |
| 3 | R3 systemd 单元 | M | 可部署性(需拍板 apply 形态) |
| 4 | R5 bundle 占位符 | S | 正确性 |
| 5 | R6 exit→Err | S | 可测试性 |
| 6 | R4 api 认证+默认 | M | 安全 |
| 7 | R7/R9/R10/R11 | S/M | 优化 |

## 快速取胜

- R5(占位符)、R6(exit→Err)、R10(端口校验)、R7(`--config`→env)可低成本在首轮 PR 内完成。
- R1 修复同时解锁 R8(真实菜单测试)。

> 注:本报告未修改任何源码。修复将在您确认方案后,按 PR 流程逐条执行并各自附回归测试。

# 贡献指南 (Contributing)

感谢参与 `vagent` 的开发。

## 开发流程
1. 从 `main` 切特性分支：`git checkout -b fix/xxx`
2. 改动后确保本地门禁全绿：
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   ```
3. 提交并开 PR：**所有改动走 PR**，不直接推 `main`。
4. PR 描述用紧凑 Markdown 表格说明 Purpose/Overview/Verification。
5. CI 绿且无 open 的 issue/PR/Dependabot/Secret-scan 后，由维护者 squash 合入。

## 代码约定
- 业务逻辑不变时，仅优化结构与质量（不重构改行为）。
- 错误处理走 `anyhow::Result` 传播，**禁止** `std::process::exit` / 裸 `panic!`。
- 所有系统副作用经 `vagent-core::executor::Executor` 出口（测试用 `FakeExecutor` 注入）。
- 内核二进制下载须走完整性校验（Xray 官方 `.dgst`；sing-box 待 `gh attestation verify`）。
- 敏感值（API token）**不**写入仓库 / systemd 单元文件，由运行环境经 `Environment=` / `EnvironmentFile` 提供。

## 审计
本仓库用 `fuck-my-shit-mountain` Skill 做全量审计（代码质量 / 安全 / 架构 / 依赖 / 文档）。
P0/P1 须在合入前闭环；P2 为优化项，可延后但须记录。

## 行为准则
- 保持文明、技术导向的讨论。
- 安全漏洞请按 SECURITY.md 私下披露，勿公开 Issue。

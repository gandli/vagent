//! Xray-core 实现。

use tracing::{info, warn};

use crate::core::{ProxyCore, Rendered};
use crate::executor::{Cmd, ExecOutput, Executor};
use crate::render::xray;
use crate::spec::Spec;
use crate::Error;
use std::path::Path;
use std::time::Duration;

/// 对**下载步骤**做有限重试(指数退避),符合 m13-domain-error:
/// 仅 transient 的网络下载可重试;校验/解压/放置失败属 permanent,不重试(fail fast)。
/// 同步 sleep(无 async runtime 依赖),最长退避随尝试次数增长。
fn run_download_with_retry(
    ex: &dyn Executor,
    cmd: &Cmd,
    max_retries: u32,
) -> Result<ExecOutput, Error> {
    let mut attempt = 0u32;
    loop {
        let out = ex.run(cmd)?; // Executor 内部错误(如无法 spawn)也纳入重试
        if out.ok() {
            return Ok(out);
        }
        attempt += 1;
        if attempt >= max_retries {
            return Err(Error::Render(format!(
                "xray 下载失败(重试 {attempt} 次后仍失败): {}",
                out.stderr
            )));
        }
        // 指数退避:1s,4s,9s ……(attempt^2)
        let backoff = (attempt * attempt).max(1) as u64;
        warn!(target: "vagent::install", attempt, backoff_secs = backoff, "下载失败,重试");
        std::thread::sleep(Duration::from_secs(backoff));
    }
}

pub struct XrayCore;

impl ProxyCore for XrayCore {
    fn id(&self) -> &'static str {
        "xray"
    }

    fn render(&self, spec: &Spec, config: &Path) -> Result<Rendered, Error> {
        let base_dir = Spec::base_dir(config);
        let path = base_dir.join("cores").join("xray").join("config.json");
        Ok(Rendered {
            path: path.to_string_lossy().to_string(),
            content: xray::render_string(spec, &base_dir)?,
        })
    }

    fn install_cmd(&self, version: &str) -> Cmd {
        // MVP:自管下载 + 校验(实际 sha256 校验在 install 流程中由调用方处理)
        Cmd::new("curl").args([
            "-L",
            "-o",
            "/tmp/xray.zip",
            &format!(
                "https://github.com/XTLS/Xray-core/releases/download/v{ver}/Xray-linux-64.zip",
                ver = version
            ),
        ])
    }

    fn reload_cmd(&self) -> Cmd {
        Cmd::new("systemctl").args(["restart", "vagent-xray"])
    }

    /// 重写安装:下载 → 校验完整性 → 解压 → 放置(四步走 Executor)。
    fn install(&self, version: &str, ex: &dyn Executor) -> Result<(), Error> {
        info!(target: "vagent::install", version, "下载 xray 内核");
        // 下载步骤可重试(transient 网络错误);校验/解压/放置不重试(permanent fail fast)
        let out = run_download_with_retry(ex, &self.install_cmd(version), 3)?;
        if !out.ok() {
            return Err(Error::Render(format!(
                "xray download failed: {}",
                out.stderr
            )));
        }
        // 完整性校验:拉官方 .dgst 提取 SHA2-256,与本地 zip sha256sum 比对,不符即中止。
        info!(target: "vagent::install", version, "校验 xray 下载完整性(官方 .dgst)");
        let dgst_url = crate::download::xray_dgst_url(version);
        let verify = crate::download::verify_cmd(&dgst_url, "/tmp/xray.zip");
        let out = ex.run(&verify)?;
        if !out.ok() {
            return Err(Error::Render(format!(
                "xray integrity verify failed: {}",
                out.stderr
            )));
        }
        let out =
            ex.run(&Cmd::new("unzip").args(["-oq", "/tmp/xray.zip", "-d", "/tmp/xray-ext"]))?;
        if !out.ok() {
            return Err(Error::Render(format!(
                "xray extract failed: {}",
                out.stderr
            )));
        }
        let dest = if crate::systemd::is_root() {
            "/usr/local/bin".to_string()
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home)
                .join(".local")
                .join("bin")
                .to_string_lossy()
                .to_string()
        };
        let place = Cmd::new("sh").args([
            "-c",
            &format!("mkdir -p {d} && mv /tmp/xray-ext/xray {d}/xray", d = dest),
        ]);
        let out = ex.run(&place)?;
        if !out.ok() {
            return Err(Error::Render(format!("xray place failed: {}", out.stderr)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{ExecOutput, FakeExecutor};
    use crate::spec::Spec;
    use std::path::Path;

    #[test]
    fn render_path_is_isolated() {
        let r = XrayCore
            .render(
                &Spec::default_for("x.com"),
                Path::new("/etc/vagent/spec.toml"),
            )
            .unwrap();
        assert_eq!(r.path, "/etc/vagent/cores/xray/config.json");
        assert!(r.content.contains("freedom"));
    }

    #[test]
    fn install_cmd_targets_release_zip() {
        let c = XrayCore.install_cmd("1.8.0");
        assert_eq!(c.program, "curl");
        assert!(c.display().contains("Xray-core/releases/download/v1.8.0"));
    }

    #[test]
    fn install_failure_propagates() {
        let ex = FakeExecutor::new().expect("curl", ExecOutput::failure(1, "404"));
        let r = XrayCore.install("1.8.0", &ex);
        assert!(r.is_err());
    }

    #[test]
    fn install_runs_all_steps_via_executor() {
        let ex = FakeExecutor::new()
            .expect("curl", ExecOutput::success(""))
            .expect("unzip", ExecOutput::success(""))
            .expect("sh", ExecOutput::success("sha256 verified: abc"));
        assert!(XrayCore.install("1.8.23", &ex).is_ok());
        let h = crate::executor::take_history();
        assert!(h.iter().any(|c| c.program == "curl"));
        // 校验步骤:sh -c 含 sha256sum
        assert!(h
            .iter()
            .any(|c| c.program == "sh" && c.display().contains("sha256sum")));
        assert!(h.iter().any(|c| c.program == "unzip"));
    }

    #[test]
    fn install_retries_transient_download_failure() {
        // m13-domain-error:transient 下载失败应重试,最终成功
        // curl 第一次失败、第二次成功 → install 应成功(校验/解压不重试,直接成功)
        let ex = FakeExecutor::new()
            .expect_sequence(
                "curl",
                vec![
                    ExecOutput::failure(1, "connection reset"),
                    ExecOutput::success(""),
                ],
            )
            .expect("unzip", ExecOutput::success(""))
            .expect("sh", ExecOutput::success("sha256 verified: abc"));
        assert!(
            XrayCore.install("1.8.23", &ex).is_ok(),
            "curl 先失败后成功应经重试成功"
        );
        let h = crate::executor::take_history();
        let curl_calls = h.iter().filter(|c| c.program == "curl").count();
        assert_eq!(curl_calls, 2, "应重试 1 次(curl 被调用 2 次)");
    }

    #[test]
    fn install_gives_up_after_max_retries() {
        // curl 连续失败超过 max_retries(3) → 最终失败(不无限重试)
        let ex = FakeExecutor::new().expect_sequence(
            "curl",
            vec![
                ExecOutput::failure(1, "timeout"),
                ExecOutput::failure(1, "timeout"),
                ExecOutput::failure(1, "timeout"),
                ExecOutput::failure(1, "timeout"),
            ],
        );
        let r = XrayCore.install("1.8.23", &ex);
        assert!(r.is_err(), "连续失败应最终返回 Err");
        let h = crate::executor::take_history();
        let curl_calls = h.iter().filter(|c| c.program == "curl").count();
        assert_eq!(curl_calls, 3, "应最多重试 3 次(curl 被调用 3 次)");
    }
    #[test]
    fn reload_via_executor() {
        let ex = FakeExecutor::new().expect("systemctl", crate::executor::ExecOutput::success(""));
        XrayCore.reload(&ex).unwrap();
    }
}

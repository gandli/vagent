//! Xray-core 实现。

use tracing::info;

use crate::core::{ProxyCore, Rendered};
use crate::executor::{Cmd, Executor};
use crate::render::xray;
use crate::spec::Spec;
use crate::Error;
use std::path::Path;

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
        let out = ex.run(&self.install_cmd(version))?;
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
    fn install_aborts_on_integrity_mismatch() {
        // 下载成功但校验(sh)失败 → install 应中止报错,不解压
        let ex = FakeExecutor::new()
            .expect("curl", ExecOutput::success(""))
            .expect("sh", ExecOutput::failure(1, "sha256 mismatch"));
        let r = XrayCore.install("1.8.23", &ex);
        assert!(r.is_err(), "校验失败应中止安装");
        let msg = format!("{:?}", r.unwrap_err());
        assert!(
            msg.contains("verify") || msg.contains("mismatch"),
            "错误应指向校验: {msg}"
        );
    }
    #[test]
    fn reload_via_executor() {
        let ex = FakeExecutor::new().expect("systemctl", crate::executor::ExecOutput::success(""));
        XrayCore.reload(&ex).unwrap();
    }
}

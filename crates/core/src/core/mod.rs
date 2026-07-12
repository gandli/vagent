//! ProxyCore:双核(Xray / sing-box)统一抽象。
//! 加协议=给对应 core 的 render 加分支,不动主流程。
//! 所有系统副作用经 Executor 出口。

pub mod singbox;
pub mod xray;

pub use singbox::SingboxCore;
pub use xray::XrayCore;

use crate::executor::Executor;
use crate::spec::Spec;
use crate::Error;
use std::path::Path;

/// 渲染产物:落盘路径 + 内容。
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub path: String,
    pub content: String,
}

/// 内核运行态。
#[derive(Debug, Clone, PartialEq)]
pub struct CoreStatus {
    pub running: bool,
    pub version: Option<String>,
}

/// 统一内核接口。
pub trait ProxyCore {
    /// 内核标识:"xray" / "singbox"
    fn id(&self) -> &'static str;

    /// 把 Spec 渲染成该内核的配置文件(纯函数,不落盘)。
    fn render(&self, spec: &Spec, config: &Path) -> Result<Rendered, Error>;

    /// 构造安装命令(不执行)。
    fn install_cmd(&self, version: &str) -> crate::executor::Cmd;

    /// 构造重载命令(不执行)。
    fn reload_cmd(&self) -> crate::executor::Cmd;

    /// systemd 服务名(默认 vagent-<id>)。
    fn service_name(&self) -> String {
        format!("vagent-{}", self.id())
    }

    /// 生命周期命令拼装(纯函数,便于单测)。
    fn lifecycle_cmd(&self, action: &str) -> crate::executor::Cmd {
        crate::executor::Cmd::new("systemctl").args([action, &self.service_name()])
    }

    /// 执行生命周期动作(start/stop/restart/enable/disable/status)。
    fn lifecycle(&self, action: &str, ex: &dyn Executor) -> Result<String, Error> {
        let out = ex.run(&self.lifecycle_cmd(action))?;
        if out.ok() {
            Ok(out.stdout)
        } else {
            Err(Error::Render(format!(
                "{} {} failed: {}",
                self.id(),
                action,
                out.stderr
            )))
        }
    }

    fn start(&self, ex: &dyn Executor) -> Result<(), Error> {
        self.lifecycle("start", ex).map(|_| ())
    }
    fn stop(&self, ex: &dyn Executor) -> Result<(), Error> {
        self.lifecycle("stop", ex).map(|_| ())
    }
    fn restart(&self, ex: &dyn Executor) -> Result<(), Error> {
        self.lifecycle("restart", ex).map(|_| ())
    }
    fn enable(&self, ex: &dyn Executor) -> Result<(), Error> {
        self.lifecycle("enable", ex).map(|_| ())
    }
    fn disable(&self, ex: &dyn Executor) -> Result<(), Error> {
        self.lifecycle("disable", ex).map(|_| ())
    }

    /// 执行安装(经 Executor)。
    fn install(&self, version: &str, ex: &dyn Executor) -> Result<(), Error> {
        let out = ex.run(&self.install_cmd(version))?;
        if out.ok() {
            Ok(())
        } else {
            Err(Error::Render(format!(
                "{} install failed: {}",
                self.id(),
                out.stderr
            )))
        }
    }

    /// 执行重载(经 Executor)。
    fn reload(&self, ex: &dyn Executor) -> Result<(), Error> {
        let out = ex.run(&self.reload_cmd())?;
        if out.ok() {
            Ok(())
        } else {
            Err(Error::Render(format!(
                "{} reload failed: {}",
                self.id(),
                out.stderr
            )))
        }
    }
}

/// 渲染所有需要的内核配置(纯函数,不落盘,可单测)。
/// 内核启用 = 显式 cores 开关 OR 存在对应协议用户(自动启用)。
/// config 用于推导派生目录锚点(支持普通用户安装)。
pub fn plan(spec: &Spec, config: &Path) -> Result<Vec<Rendered>, Error> {
    let mut out = vec![];
    if spec.cores.xray || spec.needs_xray() {
        out.push(XrayCore.render(spec, config)?);
    }
    if spec.cores.singbox || spec.needs_singbox() {
        out.push(SingboxCore.render(spec, config)?);
    }
    Ok(out)
}

/// 应用:渲染 → 写隔离路径 → 经 Executor 重载。
/// 真实写盘(需 root)不在单测范围,由 VPS E2E 覆盖。
pub fn apply(spec: &Spec, config: &Path, ex: &dyn Executor) -> Result<(), Error> {
    for r in plan(spec, config)? {
        tracing::info!(target: "vagent::apply", path = %r.path, "写入内核配置");
        write_rendered(&r)?;
        tracing::info!(target: "vagent::apply", path = %r.path, "重载内核");
        reload_by_path(&r.path, ex)?;
    }
    Ok(())
}

/// 根据落盘路径决定重载哪个内核的 systemd 单元。
fn reload_by_path(path: &str, ex: &dyn Executor) -> Result<(), Error> {
    if path.contains("xray") {
        XrayCore.reload(ex)
    } else if path.contains("singbox") {
        SingboxCore.reload(ex)
    } else {
        Ok(())
    }
}

fn write_rendered(r: &Rendered) -> Result<(), Error> {
    if let Some(parent) = Path::new(&r.path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&r.path, &r.content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{take_history, ExecOutput, FakeExecutor};
    use crate::spec::Spec;
    use std::path::Path;

    #[test]
    fn render_all_respects_enabled_cores() {
        let mut spec = Spec::default_for("x.com");
        spec.cores.singbox = true;
        let rs = plan(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        assert_eq!(rs.len(), 2);
        assert!(rs.iter().any(|r| r.path.contains("xray")));
        assert!(rs.iter().any(|r| r.path.contains("singbox")));
    }

    #[test]
    fn render_all_single_core_when_singbox_off() {
        let spec = Spec::default_for("x.com");
        let rs = plan(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        assert_eq!(rs.len(), 1);
        assert!(rs[0].path.contains("xray"));
    }

    #[test]
    fn apply_reloads_enabled_cores() {
        take_history();
        let spec = Spec::default_for("x.com");
        let ex = FakeExecutor::new().expect("systemctl", ExecOutput::success(""));
        // 用 plan(纯渲染)+ 手动 reload 验证逻辑,不触发真实写盘
        let rendered = plan(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        for r in &rendered {
            reload_by_path(&r.path, &ex).unwrap();
        }
        let h = take_history();
        assert!(h
            .iter()
            .any(|c| c.program == "systemctl" && c.args.contains(&"vagent-xray".to_string())));
    }

    #[test]
    fn lifecycle_cmd_builds_systemctl() {
        let c = XrayCore.lifecycle_cmd("restart");
        assert_eq!(c.program, "systemctl");
        assert!(c.args.contains(&"restart".to_string()));
        assert!(c.args.contains(&"vagent-xray".to_string()));
    }

    #[test]
    fn lifecycle_start_via_executor() {
        let ex = FakeExecutor::new().expect("systemctl", ExecOutput::success(""));
        XrayCore.start(&ex).unwrap();
    }

    #[test]
    fn lifecycle_failure_propagates() {
        let ex = FakeExecutor::new().expect("systemctl", ExecOutput::failure(1, "unit not found"));
        assert!(SingboxCore.enable(&ex).is_err());
    }

    #[test]
    fn service_name_default() {
        assert_eq!(XrayCore.service_name(), "vagent-xray");
        assert_eq!(SingboxCore.service_name(), "vagent-singbox");
    }
}

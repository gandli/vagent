//! 服务单元生成(纯函数,可单测)。
//! 支持 systemd(主流)与 openrc(Alpine)。真实 enable 在 VPS 上执行。

use crate::Error;

/// init 系统类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    Systemd,
    Openrc,
}

impl std::str::FromStr for InitSystem {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "systemd" => Ok(InitSystem::Systemd),
            "openrc" => Ok(InitSystem::Openrc),
            other => Err(format!("未知 init 系统: {other}")),
        }
    }
}

/// 生成指定内核的服务单元(按 init 系统分支)。
pub fn unit_for(init: InitSystem, core: &str, binary_path: &str, config: &str) -> String {
    match init {
        InitSystem::Systemd => systemd_unit(core, binary_path, config),
        InitSystem::Openrc => openrc_script(core, binary_path, config),
    }
}

fn systemd_unit(core: &str, binary_path: &str, config: &str) -> String {
    format!(
        "[Unit]\n\
Description=vagent {core} managed by vagent\n\
After=network.target\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart={bin} apply --config {cfg}\n\
Restart=on-failure\n\
RestartSec=3\n\
User=root\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n",
        core = core,
        bin = binary_path,
        cfg = config
    )
}

fn openrc_script(core: &str, binary_path: &str, config: &str) -> String {
    let name = service_name(core);
    format!(
        "#!/sbin/openrc-run\n\
command=\"{bin}\"\n\
command_args=\"apply --config {cfg}\"\n\
command_background=true\n\
pidfile=\"/run/{name}.pid\"\n\
description=\"vagent {core} managed by vagent\"\n",
        bin = binary_path,
        cfg = config,
        core = core,
        name = name
    )
}

/// 服务名(跨 init 系统统一)。
pub fn service_name(core: &str) -> String {
    format!("vagent-{core}")
}

/// 生成 vagent-xray.service 单元文本(systemd,向后兼容)。
pub fn xray_unit(binary_path: &str, config: &str) -> String {
    systemd_unit("xray", binary_path, config)
}

/// 生成 vagent-api.service(loopback 面板 API,systemd)。
pub fn api_unit(binary_path: &str) -> String {
    format!(
        "[Unit]\n\
Description=vagent local API (loopback panel)\n\
After=network.target\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart={bin}\n\
Restart=on-failure\n\
RestartSec=3\n\
User=root\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n",
        bin = binary_path
    )
}

/// 写单元到对应系统目录(需 root,不在单测范围)。
pub fn install_unit(init: InitSystem, core: &str, content: &str) -> Result<(), Error> {
    let path = match init {
        InitSystem::Systemd => format!("/etc/systemd/system/{}.service", service_name(core)),
        InitSystem::Openrc => format!("/etc/init.d/{}", service_name(core)),
    };
    std::fs::write(&path, content)?;
    Ok(())
}

/// 卸载步骤:停用服务 → 禁用 → 删单元 → reload。返回命令列表(纯函数,可单测)。
pub fn uninstall_cmds() -> Vec<crate::executor::Cmd> {
    use crate::executor::Cmd;
    let services = ["vagent-xray", "vagent-singbox", "vagent-api"];
    let mut cmds = vec![];
    for s in services {
        cmds.push(Cmd::new("systemctl").args(["stop", s]));
        cmds.push(Cmd::new("systemctl").args(["disable", s]));
        cmds.push(Cmd::new("rm").args(["-f", &format!("/etc/systemd/system/{s}.service")]));
    }
    cmds.push(Cmd::new("systemctl").args(["daemon-reload"]));
    cmds
}

/// 执行卸载(经 Executor)。best-effort:单条失败不中断(服务可能本就不存在)。
pub fn uninstall(ex: &dyn crate::executor::Executor) -> Result<(), Error> {
    for c in uninstall_cmds() {
        let _ = ex.run(&c); // best-effort
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn systemd_xray_contains_execstart() {
        let u = systemd_unit("xray", "/usr/local/bin/vagent", "/etc/vagent/spec.toml");
        assert!(u.contains("Description=vagent xray"));
        assert!(u.contains("ExecStart=/usr/local/bin/vagent apply --config /etc/vagent/spec.toml"));
        assert!(u.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn openrc_script_contains_command() {
        let u = openrc_script("xray", "/usr/local/bin/vagent", "/etc/vagent/spec.toml");
        assert!(u.contains("#!/sbin/openrc-run"));
        assert!(u.contains("command=\"/usr/local/bin/vagent\""));
        assert!(u.contains("command_args=\"apply --config /etc/vagent/spec.toml\""));
    }

    #[test]
    fn unit_for_dispatches_by_init() {
        let s = unit_for(InitSystem::Systemd, "xray", "/b/v", "/etc/vagent/spec.toml");
        assert!(s.contains("[Unit]"));
        let o = unit_for(InitSystem::Openrc, "xray", "/b/v", "/etc/vagent/spec.toml");
        assert!(o.contains("openrc-run"));
    }

    #[test]
    fn init_from_str() {
        assert_eq!(
            InitSystem::from_str("systemd").unwrap(),
            InitSystem::Systemd
        );
        assert_eq!(InitSystem::from_str("openrc").unwrap(), InitSystem::Openrc);
        assert!(InitSystem::from_str("x").is_err());
    }

    #[test]
    fn service_name_uniform() {
        assert_eq!(service_name("xray"), "vagent-xray");
    }

    #[test]
    fn api_unit_looback_service() {
        let u = api_unit("/usr/local/bin/vagent-api");
        assert!(u.contains("vagent local API"));
        assert!(u.contains("ExecStart=/usr/local/bin/vagent-api"));
    }

    #[test]
    fn uninstall_cmds_cover_all_services() {
        let cmds = uninstall_cmds();
        let all = cmds
            .iter()
            .map(|c| c.display())
            .collect::<Vec<_>>()
            .join("\n");
        for s in ["vagent-xray", "vagent-singbox", "vagent-api"] {
            assert!(all.contains(&format!("stop {s}")));
            assert!(all.contains(&format!("disable {s}")));
        }
        assert!(all.contains("daemon-reload"));
    }

    #[test]
    fn uninstall_best_effort_ignores_failures() {
        use crate::executor::{ExecOutput, FakeExecutor};
        let ex = FakeExecutor::new()
            .expect("systemctl", ExecOutput::failure(1, "not found"))
            .expect("rm", ExecOutput::failure(1, "no file"));
        assert!(uninstall(&ex).is_ok());
    }
}

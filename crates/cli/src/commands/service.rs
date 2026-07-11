//! 服务单元管理:生成并安装 systemd / openrc 单元。
//! init 默认 systemd;Alpine 用 openrc。

use std::str::FromStr;
use vagent_core::systemd::{self, InitSystem};

const BIN: &str = "/usr/local/bin/vagent";
const CONFIG: &str = "/etc/vagent/spec.toml";

/// 为指定内核生成并打印单元内容(不落盘)。
pub fn show(core: &str, init: &str) -> anyhow::Result<()> {
    let init = InitSystem::from_str(init).map_err(|e| anyhow::anyhow!(e))?;
    let unit = systemd::unit_for(init, core, BIN, CONFIG);
    println!("{unit}");
    Ok(())
}

/// 生成并写入单元到系统目录(需 root)。
pub fn install(core: &str, init: &str) -> anyhow::Result<()> {
    let init = InitSystem::from_str(init).map_err(|e| anyhow::anyhow!(e))?;
    let unit = systemd::unit_for(init, core, BIN, CONFIG);
    systemd::install_unit(init, core, &unit).map_err(|e| anyhow::anyhow!(e))?;
    println!("已安装 {core} 单元 ({init:?})");
    Ok(())
}

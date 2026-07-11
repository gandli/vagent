//! 卸载:停用并删除所有 vagent systemd 服务。
//! 默认保留 /etc/vagent 配置;--purge 时一并删除。

use std::path::Path;
use vagent_core::executor::RealExecutor;
use vagent_core::systemd;

pub fn run(purge: bool) -> anyhow::Result<()> {
    systemd::uninstall(&RealExecutor).map_err(|e| anyhow::anyhow!(e))?;
    println!("已停用并移除 vagent systemd 服务");
    if purge {
        let dir = Path::new("/etc/vagent");
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
            println!("已删除配置目录 /etc/vagent");
        }
    } else {
        println!("配置目录 /etc/vagent 已保留(如需删除请加 --purge)");
    }
    Ok(())
}

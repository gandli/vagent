mod cli;
mod commands;

use clap::Parser;
use cli::Cli;
use std::path::PathBuf;
use vagent_core::core::apply as core_apply;
use vagent_core::Spec;

fn resolve_config() -> PathBuf {
    // 零命令行参数:配置路径仅来自 VAGENT_CONFIG 环境变量或默认位置
    std::env::var("VAGENT_CONFIG")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(Spec::default_config_path)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = resolve_config();

    // 非交互 apply 模式(systemd 单元调用):渲染+写盘+重载,不进菜单
    if cli.apply {
        let spec =
            vagent_core::load_spec(&config).map_err(|e| anyhow::anyhow!("加载配置失败: {e}"))?;
        core_apply(
            &spec,
            &config,
            &vagent_core::executor::RealExecutor as &dyn vagent_core::executor::Executor,
        )?;
        println!("vagent --apply 已渲染并应用配置");
        return Ok(());
    }

    // 直接进入交互菜单(所有操作在菜单内完成)
    commands::menu::run(&config)?;
    Ok(())
}

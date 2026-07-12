mod cli;
mod commands;

use clap::Parser;
use cli::Cli;
use std::path::PathBuf;
use vagent_core::core::{ProxyCore, SingboxCore, XrayCore};
use vagent_core::load_spec;
use vagent_core::Spec;

fn default_config() -> PathBuf {
    Spec::default_config_path()
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = cli
        .config
        .or_else(|| std::env::var("VAGENT_CONFIG").ok().map(PathBuf::from))
        .unwrap_or_else(default_config);

    // 非交互渲染模式(脚本 / CI):--render xray|singbox
    if let Some(core) = cli.render.as_deref() {
        let spec = load_spec(&config)?;
        let rendered = match core.to_lowercase().as_str() {
            "xray" => XrayCore.render(&spec, &config)?,
            "singbox" => SingboxCore.render(&spec, &config)?,
            other => return Err(anyhow::anyhow!("未知内核: {other}")),
        };
        print!("{}", rendered.content);
        return Ok(());
    }

    // 非交互 Reality 密钥生成(脚本 / CI)
    if cli.reality_gen {
        commands::reality::run(&config, None)?;
        return Ok(());
    }

    // 无上述 flag:直接进入交互菜单(所有操作在菜单内完成)
    commands::menu::run(&config)?;
    Ok(())
}

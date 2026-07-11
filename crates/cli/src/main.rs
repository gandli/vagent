mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::PathBuf;
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

    // 无子命令 / `vagent menu` → 进入交互菜单
    match cli.command {
        None | Some(Commands::Menu) => {
            commands::menu::run(&config)?;
        }
        Some(cmd) => match cmd {
            Commands::Init { domain, dry_run } => {
                commands::init::run(domain.as_deref().unwrap_or("example.com"), &config, dry_run)?
            }
            Commands::Status => commands::status::run(&config)?,
            Commands::Render { core, out } => {
                commands::render::run(&config, &core, out.as_deref())?
            }
            Commands::Apply { dry_run } => commands::apply::run(&config, dry_run)?,
            Commands::UserAdd {
                name,
                port,
                protocol,
                transport,
            } => commands::user::add(&config, &name, port, &protocol, &transport)?,
            Commands::UserList => commands::user::list(&config)?,
            Commands::UserDel { name } => commands::user::del(&config, &name)?,
            Commands::UserLink { name } => commands::user::link(&config, &name)?,
            Commands::CoreInstall { core, version } => {
                commands::core_install::run(&core, &version)?
            }
            Commands::Core { action, core } => commands::core_ctl::run(&core, &action)?,
            Commands::Route { action, value } => {
                commands::route::run(&config, &action, value.as_deref())?
            }
            Commands::CertIssue { domain, ca, dns } => {
                commands::cert::issue(&domain, &ca, dns.as_deref(), &config)?
            }
            Commands::CertRenew => commands::cert::renew()?,
            Commands::Service { action, core, init } => match action.as_str() {
                "show" => commands::service::show(&core, &init)?,
                "install" => commands::service::install(&core, &init)?,
                other => return Err(anyhow::anyhow!("未知 service 动作: {other}")),
            },
            Commands::RealityGen { name } => commands::reality::run(&config, name.as_deref())?,
            Commands::RealityScan { public_ip } => commands::scan::run(&config, &public_ip)?,
            Commands::Uninstall { purge } => commands::uninstall::run(purge)?,
            Commands::Subscribe { sign } => commands::subscribe::run(&config, sign)?,
            Commands::Menu => commands::menu::run(&config)?,
        },
    }
    Ok(())
}

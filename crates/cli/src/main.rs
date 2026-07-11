mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::PathBuf;
use vagent_core::Spec;

fn default_config() -> PathBuf {
    PathBuf::from("/etc/vagent/spec.toml")
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = cli
        .config
        .or_else(|| std::env::var("VAGENT_CONFIG").ok().map(PathBuf::from))
        .unwrap_or_else(default_config);
    match cli.command {
        Commands::Init { domain, dry_run } => {
            commands::init::run(domain.as_deref().unwrap_or("example.com"), &config, dry_run)?
        }
        Commands::Status => commands::status::run(&config)?,
        Commands::Render => commands::render::run(&config)?,
        Commands::Apply { dry_run } => commands::apply::run(&config, dry_run)?,
        Commands::UserAdd {
            name,
            port,
            protocol,
        } => commands::user::add(&config, &name, port, &protocol)?,
        Commands::UserList => commands::user::list(&config)?,
        Commands::UserDel { name } => commands::user::del(&config, &name)?,
        Commands::UserLink { name } => commands::user::link(&config, &name)?,
        Commands::CoreInstall { core, version } => commands::core_install::run(&core, &version)?,
        Commands::Core { action, core } => commands::core_ctl::run(&core, &action)?,
        Commands::Route { action, value } => {
            commands::route::run(&config, &action, value.as_deref())?
        }
        Commands::CertIssue { domain, ca, dns } => {
            commands::cert::issue(&domain, &ca, dns.as_deref())?
        }
        Commands::CertRenew => commands::cert::renew()?,
    }
    Ok(())
}

// 确保 Spec 在二进制中可用(供未来命令直接构造)。
#[allow(dead_code)]
fn _spec_marker() -> Spec {
    Spec::default_for("x")
}

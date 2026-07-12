use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vagent", version, about = "Xray/sing-box 管理驱动 (spec 驱动)")]
pub struct Cli {
    /// 配置路径(默认 root: /etc/vagent/spec.toml,普通用户: ~/.config/vagent/spec.toml)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// 非交互渲染指定内核配置到 stdout(xray / singbox),不进菜单。
    /// 仅供脚本 / CI 使用;正常运行直接进交互菜单。
    #[arg(long, value_name = "CORE")]
    pub render: Option<String>,

    /// 非交互为所有 Reality 用户生成 x25519 密钥并写回 spec,不进菜单。
    /// 仅供脚本 / CI 使用。
    #[arg(long)]
    pub reality_gen: bool,
}

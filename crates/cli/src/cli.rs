use clap::Parser;

#[derive(Parser)]
#[command(name = "vagent", version, about = "Xray/sing-box 管理驱动 (spec 驱动)")]
pub struct Cli {
    /// 非交互模式:渲染配置并写盘后退出(供 systemd 单元调用),不进入交互菜单。
    #[arg(long, default_value_t = false)]
    pub apply: bool,
}

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vagent", version, about = "Xray/sing-box 管理驱动 (spec 驱动)")]
pub struct Cli {
    /// 配置路径(默认 /etc/vagent/spec.toml)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 初始化默认配置
    Init {
        /// 域名
        #[arg(long)]
        domain: Option<String>,
        /// 只渲染不落盘
        #[arg(long)]
        dry_run: bool,
    },
    /// 查看状态(从 spec 读取,不反推 JSON)
    Status,
    /// 渲染 Xray 配置(MVP: VLESS+Reality)
    Render,
    /// 应用配置:渲染→落盘→重载启用内核
    Apply {
        /// 只渲染并打印,不落盘/不重载
        #[arg(long)]
        dry_run: bool,
    },
    /// 新增用户(默认 VLESS+Reality,可选其他协议)
    UserAdd {
        /// 用户名
        name: String,
        /// 端口
        #[arg(long, default_value_t = 443)]
        port: u16,
        /// 协议:vless/vmess/trojan/hysteria2/tuic/naive
        #[arg(long, default_value = "vless")]
        protocol: String,
        /// 传输层:tcp/ws/grpc/xhttp(默认 tcp;reality 强制 tcp)
        #[arg(long, default_value = "tcp")]
        transport: String,
    },
    /// 列出所有用户
    UserList,
    /// 删除用户(按名字)
    UserDel {
        /// 用户名
        name: String,
    },
    /// 生成用户的分享链接
    UserLink {
        /// 用户名
        name: String,
    },
    /// 安装内核二进制
    CoreInstall {
        /// 内核:xray / singbox
        #[arg(long, default_value = "xray")]
        core: String,
        /// 版本(不含 v 前缀)
        #[arg(long, default_value = "1.8.0")]
        version: String,
    },
    /// 内核生命周期:start/stop/restart/enable/disable
    Core {
        /// 动作
        action: String,
        /// 内核:xray / singbox
        #[arg(long, default_value = "xray")]
        core: String,
    },
    /// 分流规则:block/direct/warp <域名> | ads/bt [on|off] | list
    Route {
        /// 动作:block/direct/warp/ads/bt/list
        action: String,
        /// 域名(block/direct/warp)或开关(ads/bt: on|off)
        value: Option<String>,
    },
    /// 签发 TLS 证书(acme.sh)
    CertIssue {
        /// 域名
        domain: String,
        /// CA:letsencrypt / zerossl / buypass
        #[arg(long, default_value = "letsencrypt")]
        ca: String,
        /// DNS 验证 hook(如 dns_cf),不填则用 standalone
        #[arg(long)]
        dns: Option<String>,
    },
    /// 续期所有 TLS 证书
    CertRenew,
    /// 服务单元:生成/安装 systemd / openrc 单元
    Service {
        /// 动作:show / install
        action: String,
        /// 内核:xray / singbox / api
        #[arg(long, default_value = "xray")]
        core: String,
        /// init 系统:systemd / openrc
        #[arg(long, default_value = "systemd")]
        init: String,
    },
    /// 生成 Reality 密钥(xray x25519),写入 spec
    RealityGen {
        /// 用户名(不填则对所有 Reality 用户)
        name: Option<String>,
    },
    /// 卸载:停用并删除所有 vagent 服务
    Uninstall {
        /// 一并删除 /etc/vagent 配置目录
        #[arg(long)]
        purge: bool,
    },
}

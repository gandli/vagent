//! 交互式菜单(对标 v2ray-agent 的 vasma)。
//! `vagent` 不带任何参数即进入本菜单;所有设定操作在菜单内完成,不依赖命令行参数。
//! 底层复用各 commands 模块的函数,菜单只负责读 stdin / 选择。

use std::path::Path;

use dialoguer::{Input, Select};

use crate::commands;

fn prompt_text(msg: &str, default: &str) -> String {
    Input::<String>::new()
        .with_prompt(msg)
        .default(default.to_string())
        .interact_text()
        .unwrap_or_else(|_| default.to_string())
}

fn prompt_optional(msg: &str) -> Option<String> {
    let s = Input::<String>::new()
        .with_prompt(msg)
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();
    if s.trim().is_empty() {
        None
    } else {
        Some(s.trim().to_string())
    }
}

/// 主菜单。config 为当前 spec 路径。
pub fn run(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = [
            "用户管理",
            "内核管理",
            "分流规则",
            "证书管理",
            "服务管理",
            "Reality",
            "订阅管理",
            "应用配置 (apply)",
            "查看状态",
            "卸载",
            "退出",
        ];
        let sel = Select::new()
            .with_prompt("vagent 管理菜单")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None);

        match sel {
            Some(0) => user_menu(config)?,
            Some(1) => core_menu()?,
            Some(2) => route_menu(config)?,
            Some(3) => cert_menu(config)?,
            Some(4) => service_menu()?,
            Some(5) => reality_menu(config)?,
            Some(6) => subscribe_menu(config)?,
            Some(7) => {
                println!("== 应用配置 ==");
                commands::apply::run(config, false)?;
            }
            Some(8) => commands::status::run(config)?,
            Some(9) => {
                println!("== 卸载 ==");
                let purge = dialoguer::Confirm::new()
                    .with_prompt("同时删除配置目录?")
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                commands::uninstall::run(purge)?;
            }
            Some(10) | None => {
                println!("再见。");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn user_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["新增用户", "列出用户", "删除用户", "生成分享链接", "返回"];
        match Select::new()
            .with_prompt("用户管理")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let name = prompt_text("用户名", "alice");
                let port_s = prompt_text("端口", "443");
                let port: u16 = port_s.trim().parse().unwrap_or(443);
                let proto = select_one(
                    "协议",
                    &["vless", "vmess", "trojan", "hysteria2", "tuic", "naive"],
                    0,
                );
                let transport = if proto == "vless" {
                    "tcp".to_string() // Reality 强制 tcp,菜单直接定
                } else {
                    select_one("传输层", &["tcp", "ws", "grpc", "xhttp"], 0)
                };
                commands::user::add(config, &name, port, &proto, &transport)?;
            }
            Some(1) => commands::user::list(config)?,
            Some(2) => {
                let name = prompt_text("要删除的用户名", "");
                if !name.is_empty() {
                    commands::user::del(config, &name)?;
                }
            }
            Some(3) => {
                let name = prompt_text("要生成链接的用户名", "");
                if !name.is_empty() {
                    commands::user::link(config, &name)?;
                }
            }
            Some(4) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn core_menu() -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["安装 Xray", "安装 Sing-box", "启停/重启内核", "返回"];
        match Select::new()
            .with_prompt("内核管理")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let ver = prompt_text("Xray 版本(不含 v)", "1.8.23");
                commands::core_install::run("xray", &ver)?;
            }
            Some(1) => {
                let ver = prompt_text("Sing-box 版本(不含 v)", "1.10.0");
                commands::core_install::run("singbox", &ver)?;
            }
            Some(2) => {
                let core = select_one("内核", &["xray", "singbox"], 0);
                let action = select_one(
                    "动作",
                    &["start", "stop", "restart", "enable", "disable"],
                    2,
                );
                commands::core_ctl::run(&core, &action)?;
            }
            Some(3) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn route_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = [
            "加入黑名单域名",
            "加入直连白名单",
            "加入 WARP 分流",
            "广告拦截 开/关",
            "BT 阻断 开/关",
            "查看当前规则",
            "返回",
        ];
        match Select::new()
            .with_prompt("分流规则")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let d = prompt_text("黑名单域名", "");
                if !d.is_empty() {
                    commands::route::run(config, "block", Some(&d))?;
                }
            }
            Some(1) => {
                let d = prompt_text("直连白名单域名", "");
                if !d.is_empty() {
                    commands::route::run(config, "direct", Some(&d))?;
                }
            }
            Some(2) => {
                let d = prompt_text("WARP 分流域名", "");
                if !d.is_empty() {
                    commands::route::run(config, "warp", Some(&d))?;
                }
            }
            Some(3) => {
                commands::route::run(config, "ads", Some("on"))?;
            }
            Some(4) => {
                commands::route::run(config, "bt", Some("on"))?;
            }
            Some(5) => commands::route::run(config, "list", None)?,
            Some(6) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn cert_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["签发证书", "续期所有证书", "返回"];
        match Select::new()
            .with_prompt("证书管理")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let domain = prompt_text("证书域名", "");
                if domain.is_empty() {
                    continue;
                }
                let ca = select_one("CA", &["letsencrypt", "zerossl", "buypass"], 0);
                let dns = prompt_optional("DNS hook(如 dns_cf,留空走 standalone)");
                commands::cert::issue(&domain, &ca, dns.as_deref(), config)?;
            }
            Some(1) => commands::cert::renew()?,
            Some(2) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn service_menu() -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["安装 systemd 单元", "查看单元内容", "返回"];
        match Select::new()
            .with_prompt("服务管理")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let core = select_one("内核", &["xray", "singbox"], 0);
                commands::service::install(&core, "systemd")?;
            }
            Some(1) => {
                let core = select_one("内核", &["xray", "singbox"], 0);
                commands::service::show(&core, "systemd")?;
            }
            Some(2) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn subscribe_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = [
            "生成订阅链接(多用户 bundle)",
            "生成签名订阅(可识别/吊销)",
            "返回",
        ];
        match Select::new()
            .with_prompt("订阅管理")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => commands::subscribe::run(config, false)?,
            Some(1) => commands::subscribe::run(config, true)?,
            Some(2) | None => break,
            _ => {}
        }
    }
    Ok(())
}
fn reality_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["生成 Reality 密钥", "扫描可用 SNI", "返回"];
        match Select::new()
            .with_prompt("Reality")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(0) => {
                let name = prompt_optional("用户名(留空=所有 Reality 用户)");
                commands::reality::run(config, name.as_deref())?;
            }
            Some(1) => {
                let ip = prompt_text("本机公网 IP", "");
                if !ip.is_empty() {
                    commands::scan::run(config, &ip)?;
                }
            }
            Some(2) | None => break,
            _ => {}
        }
    }
    Ok(())
}

/// 通用单选,返回选中项的字符串。
fn select_one(prompt: &str, options: &[&str], default: usize) -> String {
    let idx = Select::new()
        .with_prompt(prompt)
        .items(options)
        .default(default.min(options.len().saturating_sub(1)))
        .interact_opt()
        .unwrap_or(None)
        .unwrap_or(default.min(options.len().saturating_sub(1)));
    options[idx].to_string()
}

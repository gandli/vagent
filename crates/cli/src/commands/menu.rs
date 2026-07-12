//! 交互式菜单(对标 v2ray-agent 的 vasma)。
//! `vagent` 零命令行参数即进入本菜单;所有设定操作在菜单内完成,不依赖命令行参数。
//! 底层复用各 commands 模块的函数,菜单只负责读 stdin / 选择。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::Path;

use dialoguer::{Confirm, Input, Select};

use crate::commands;
use vagent_core::{save_spec, Spec};

// 测试输入注入:环境变量 VAGENT_TEST_INPUT(换行分隔)。
// 每行依次是:数字=菜单选择索引,文本=Input/Confirm 的答案。
// 生产环境不设置此变量,菜单走正常 dialoguer 交互。
thread_local! {
    static TEST_INPUT: RefCell<VecDeque<String>> = {
        let s = std::env::var("VAGENT_TEST_INPUT").unwrap_or_default();
        let q: VecDeque<String> = if s.is_empty() {
            VecDeque::new()
        } else {
            s.split('\n').map(|x| x.to_string()).collect()
        };
        RefCell::new(q)
    };
}

/// 取下一行测试输入(若有)。
fn next_test_line() -> Option<String> {
    TEST_INPUT.with(|q| q.borrow_mut().pop_front())
}

/// 菜单单选:优先消费测试输入(数字索引或匹配文本),否则走 dialoguer。
/// 非交互模式(VAGENT_TEST_INPUT 已设置但输入耗尽)返回 None,由菜单循环退出,
/// 避免 assert_cmd/管道下 dialoguer 误选默认项。
fn menu_select(prompt: &str, items: &[&str]) -> Option<usize> {
    if let Some(line) = next_test_line() {
        let line = line.trim();
        if let Ok(idx) = line.parse::<usize>() {
            return Some(idx);
        }
        if line.is_empty() {
            return None;
        }
        return items.iter().position(|i| *i == line);
    }
    // 非交互模式:VAGENT_TEST_INPUT 存在但输入已耗尽 → 优雅退出。
    if std::env::var("VAGENT_TEST_INPUT").is_ok() {
        return None;
    }
    Select::new()
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact_opt()
        .unwrap_or(None)
}

/// 是/否确认:优先消费测试输入(y/yes/1=true,n/no/0=false),否则走 dialoguer。
fn menu_confirm(prompt: &str, default: bool) -> bool {
    if let Some(line) = next_test_line() {
        return match line.trim().to_lowercase().as_str() {
            "y" | "yes" | "1" | "true" => true,
            "n" | "no" | "0" | "false" => false,
            _ => default,
        };
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()
        .unwrap_or(default)
}

fn prompt_text(msg: &str, default: &str) -> String {
    if let Some(line) = next_test_line() {
        if line.trim().is_empty() {
            return default.to_string();
        }
        return line;
    }
    Input::<String>::new()
        .with_prompt(msg)
        .default(default.to_string())
        .interact_text()
        .unwrap_or_else(|_| default.to_string())
}

fn prompt_optional(msg: &str) -> Option<String> {
    if let Some(line) = next_test_line() {
        let s = line.trim();
        return if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
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

/// 通用单选,返回选中项的字符串。
fn select_one(prompt: &str, options: &[&str], default: usize) -> String {
    let idx = menu_select(prompt, options).unwrap_or(default.min(options.len().saturating_sub(1)));
    options[idx.min(options.len() - 1)].to_string()
}

/// 主菜单。config 为当前 spec 路径。
/// 结构对齐 mack-a/v2ray-agent 的菜单布局(分组 + 编号语义)。
pub fn run(config: &Path) -> anyhow::Result<()> {
    // config 不存在 → 引导初始化(对标 v2ray-agent 首跑建配置)
    if !config.exists() {
        println!("未找到配置:{}", config.display());
        // 测试模式(VAGENT_TEST_INPUT 存在)下不消费测试输入,直接用默认域名,
        // 否则首跑引导会抢走菜单第一行的选择索引。
        let domain = if std::env::var("VAGENT_TEST_INPUT").is_ok() {
            "example.com".to_string()
        } else {
            prompt_text("请输入域名(如 example.com)", "example.com")
        };
        let spec = Spec::default_for(&domain);
        if let Some(parent) = config.parent() {
            std::fs::create_dir_all(parent)?;
        }
        save_spec(&spec, config)?;
        println!("已生成默认配置:{}", config.display());
    }

    // 是否已安装(有 spec 且含用户视为已装)
    let installed = std::fs::read_to_string(config)
        .map(|s| s.contains("users"))
        .unwrap_or(false);

    let items = [
        if installed {
            "安装/重新安装"
        } else {
            "安装"
        },
        "一键 Reality(无域名)",
        "Hysteria2 管理",
        "REALITY 管理",
        "Tuic 管理",
        "用户管理",
        "证书管理",
        "分流规则",
        "订阅管理",
        "内核管理 (xray / sing-box)",
        "应用配置 (apply)",
        "查看状态",
        "卸载",
        "退出",
    ];

    loop {
        println!();
        println!("==============================================================");
        println!("0. {}", items[0]);
        println!("1. {}", items[1]);
        println!("2. {}", items[2]);
        println!("3. {}", items[3]);
        println!("4. {}", items[4]);
        println!("------------------------- 工具管理 -----------------------------");
        println!("5. {}", items[5]);
        println!("6. {}", items[6]);
        println!("7. {}", items[7]);
        println!("8. {}", items[8]);
        println!("------------------------- 内核管理 -----------------------------");
        println!("9. {}", items[9]);
        println!("10. {}", items[10]);
        println!("11. {}", items[11]);
        println!("------------------------- 脚本管理 -----------------------------");
        println!("12. {}", items[12]);
        println!("13. {}", items[13]);
        println!("==============================================================");

        match menu_select("vagent 管理菜单", &items) {
            Some(0) => {
                // 安装 / 重新安装:装 xray + 应用
                commands::core_install::run("xray", "1.8.23")?;
                commands::apply::run(config, false)?;
            }
            Some(1) => reality_oneclick(config)?,
            Some(2) => proto_menu(config, "hysteria2")?,
            Some(3) => reality_menu(config)?,
            Some(4) => proto_menu(config, "tuic")?,
            Some(5) => user_menu(config)?,
            Some(6) => cert_menu(config)?,
            Some(7) => route_menu(config)?,
            Some(8) => subscribe_menu(config)?,
            Some(9) => core_menu(config)?,
            Some(10) => {
                println!("== 应用配置 ==");
                commands::apply::run(config, false)?;
            }
            Some(11) => commands::status::run(config)?,
            Some(12) => {
                println!("== 卸载 ==");
                let purge = menu_confirm("同时删除配置目录?", false);
                commands::uninstall::run(purge)?;
            }
            Some(13) | None => {
                println!("再见。");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

/// 一键 Reality:生成 Reality 用户 + 密钥 + 应用。
/// 依赖 xray 二进制生成密钥;若未安装则提示先装内核,不崩溃。
fn reality_oneclick(config: &Path) -> anyhow::Result<()> {
    println!("== 一键 Reality ==");
    let xray = commands::reality::xray_bin();
    if !std::path::Path::new(&xray).exists() {
        println!("未检测到 xray({xray}),请先在『内核管理』安装 xray 后再用一键 Reality。");
        return Ok(());
    }
    commands::user::add(config, "reality", 443, "vless", "tcp", true)?;
    commands::reality::run(config, Some("reality"))?;
    commands::apply::run(config, false)?;
    Ok(())
}

/// 某协议的用户管理(Hysteria2 / Tuic),对标 v2ray-agent 的专项管理菜单。
fn proto_menu(config: &Path, proto: &str) -> anyhow::Result<()> {
    loop {
        println!();
        let add_label = format!("新增 {proto} 用户");
        let items = [add_label.as_str(), "列出用户", "返回"];
        match menu_select(&format!("{proto} 管理"), &items) {
            Some(0) => {
                let name = prompt_text("用户名", "alice");
                let port_s = prompt_text("端口", "8443");
                let port: u16 = port_s.trim().parse().unwrap_or(8443);
                commands::user::add(config, &name, port, proto, "tcp", false)?;
            }
            Some(1) => commands::user::list(config)?,
            Some(2) | None => break,
            _ => {}
        }
    }
    Ok(())
}

fn user_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["新增用户", "列出用户", "删除用户", "生成分享链接", "返回"];
        match menu_select("用户管理", &items) {
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
                commands::user::add(config, &name, port, &proto, &transport, false)?;
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

fn core_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = ["安装 Xray", "安装 Sing-box", "启停/重启内核", "返回"];
        match menu_select("内核管理", &items) {
            Some(0) => {
                let ver = prompt_text("Xray 版本(不含 v)", "1.8.23");
                commands::core_install::run("xray", &ver)?;
                // 装完内核顺手安装 systemd 单元(对齐 v2ray-agent 装完即用)
                commands::service::install("xray", "systemd")?;
                commands::apply::run(config, false)?;
            }
            Some(1) => {
                let ver = prompt_text("Sing-box 版本(不含 v)", "1.10.0");
                commands::core_install::run("singbox", &ver)?;
                commands::service::install("singbox", "systemd")?;
                commands::apply::run(config, false)?;
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
        match menu_select("分流规则", &items) {
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
        match menu_select("证书管理", &items) {
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

fn subscribe_menu(config: &Path) -> anyhow::Result<()> {
    loop {
        println!();
        let items = [
            "生成订阅链接(多用户 bundle)",
            "生成签名订阅(可识别/吊销)",
            "返回",
        ];
        match menu_select("订阅管理", &items) {
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
        match menu_select("Reality", &items) {
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

//! 分流规则管理:增删黑名单/白名单/WARP 域名 + 广告/BT 开关。
//! 操作 spec.rules,落盘后需 `vagent apply` 生效。

use std::path::Path;
use vagent_core::{load_spec, save_spec};

fn load_or_exit(config: &Path) -> vagent_core::Spec {
    match load_spec(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("加载配置失败 {}: {e}", config.display());
            std::process::exit(1);
        }
    }
}

/// 分流规则操作。action ∈ {block,direct,warp,ads,bt,list}。
pub fn run(config: &Path, action: &str, value: Option<&str>) -> anyhow::Result<()> {
    let mut spec = load_or_exit(config);
    match action {
        "block" => {
            let d = require(value)?;
            spec.rules.domain_blocklist.push(d.to_string());
            save_spec(&spec, config)?;
            println!("已加入黑名单: {d}");
        }
        "direct" => {
            let d = require(value)?;
            spec.rules.direct_domains.push(d.to_string());
            save_spec(&spec, config)?;
            println!("已加入直连白名单: {d}");
        }
        "warp" => {
            let d = require(value)?;
            spec.rules.warp_domains.push(d.to_string());
            save_spec(&spec, config)?;
            println!("已加入 WARP 分流: {d}");
        }
        "ads" => {
            spec.rules.block_ads = parse_toggle(value)?;
            save_spec(&spec, config)?;
            println!("广告拦截: {}", onoff(spec.rules.block_ads));
        }
        "bt" => {
            spec.rules.block_bt = parse_toggle(value)?;
            save_spec(&spec, config)?;
            println!("BT 阻断: {}", onoff(spec.rules.block_bt));
        }
        "list" => {
            let r = &spec.rules;
            println!("广告拦截: {}", onoff(r.block_ads));
            println!("BT 阻断:  {}", onoff(r.block_bt));
            println!("黑名单:   {:?}", r.domain_blocklist);
            println!("白名单:   {:?}", r.direct_domains);
            println!("WARP:     {:?}", r.warp_domains);
        }
        other => return Err(anyhow::anyhow!("未知分流动作: {other}")),
    }
    Ok(())
}

fn require(value: Option<&str>) -> anyhow::Result<&str> {
    value.ok_or_else(|| anyhow::anyhow!("该动作需要一个域名参数"))
}

fn parse_toggle(value: Option<&str>) -> anyhow::Result<bool> {
    match value {
        Some("on") | Some("true") | None => Ok(true),
        Some("off") | Some("false") => Ok(false),
        Some(o) => Err(anyhow::anyhow!("无效开关: {o}(应为 on/off)")),
    }
}

fn onoff(b: bool) -> &'static str {
    if b {
        "开"
    } else {
        "关"
    }
}

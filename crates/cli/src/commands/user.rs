use std::path::Path;
use std::str::FromStr;
use vagent_core::{load_spec, save_spec, Protocol, Transport};

fn load_or_exit(config: &Path) -> vagent_core::Spec {
    match load_spec(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("加载配置失败 {}: {e}", config.display());
            std::process::exit(1);
        }
    }
}

/// reality=true 时传输强制 tcp(Reality 仅支持 tcp)。
pub fn add(
    config: &Path,
    name: &str,
    port: u16,
    protocol: &str,
    transport: &str,
    reality: bool,
) -> anyhow::Result<()> {
    let mut spec = load_or_exit(config);
    let proto = Protocol::from_str(protocol).map_err(|e| anyhow::anyhow!(e))?;
    let mut t = Transport::from_str(transport).map_err(|e| anyhow::anyhow!(e))?;
    // VLESS + Reality 时传输强制 tcp(Reality 仅 tcp)
    let reality = if matches!(proto, Protocol::Vless) {
        reality
    } else {
        false // 非 vless 不支持 reality
    };
    if reality {
        t = Transport::Tcp;
    }
    spec.users.push(vagent_core::User::new(
        name,
        proto.clone(),
        port,
        reality,
        t.clone(),
    ));
    save_spec(&spec, config)?;
    let suffix = if reality { " (Reality)" } else { "" };
    println!("已新增用户 {name} (端口 {port}, {proto} {t}{suffix})");
    Ok(())
}

pub fn list(config: &Path) -> anyhow::Result<()> {
    let spec = load_or_exit(config);
    if spec.users.is_empty() {
        println!("(无用户)");
        return Ok(());
    }
    println!(
        "{:<16} {:<10} {:<6} {:<6} UUID",
        "NAME", "PROTOCOL", "PORT", "TRANS"
    );
    for u in &spec.users {
        println!(
            "{:<16} {:<10} {:<6} {:<6} {}",
            u.name, u.protocol, u.port, u.transport, u.uuid
        );
    }
    Ok(())
}

pub fn del(config: &Path, name: &str) -> anyhow::Result<()> {
    let mut spec = load_or_exit(config);
    let n = spec.remove_user(name);
    if n == 0 {
        eprintln!("未找到用户: {name}");
        std::process::exit(1);
    }
    save_spec(&spec, config)?;
    println!("已删除用户 {name} ({n} 条)");
    Ok(())
}

pub fn link(config: &Path, name: &str) -> anyhow::Result<()> {
    let spec = load_or_exit(config);
    let user = match spec.users.iter().find(|u| u.name == name) {
        Some(u) => u,
        None => {
            eprintln!("未找到用户: {name}");
            std::process::exit(1);
        }
    };
    let l = vagent_core::subscribe::gen_user(user, &spec).map_err(|e| anyhow::anyhow!(e))?;
    println!("{l}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use vagent_core::{save_spec, Spec};

    fn fresh_cfg(p: &str) -> std::path::PathBuf {
        let cfg = Path::new(p);
        let _ = std::fs::remove_file(cfg);
        // add 假设 spec 已存在(菜单 init 创建),测试先落一个最小 spec
        save_spec(&Spec::default_for("x.com"), cfg).unwrap();
        cfg.to_path_buf()
    }

    #[test]
    fn add_vless_plain_is_not_reality() {
        // 普通 vless 用户不应被标 reality(否则渲染出占位 privateKey,xray 拒绝)
        let cfg = fresh_cfg("/tmp/vagent-test-add-plain.toml");
        add(&cfg, "alice", 443, "vless", "tcp", false).unwrap();
        let spec = load_spec(&cfg).unwrap();
        assert_eq!(spec.users.len(), 1);
        assert!(!spec.users[0].reality, "普通 vless 不应 reality=true");
        let _ = std::fs::remove_file(&cfg);
    }

    #[test]
    fn add_vless_reality_flag_sets_reality() {
        let cfg = fresh_cfg("/tmp/vagent-test-add-reality.toml");
        add(&cfg, "r", 443, "vless", "tcp", true).unwrap();
        let spec = load_spec(&cfg).unwrap();
        assert!(spec.users[0].reality, "显式 reality=true 应保留");
        let _ = std::fs::remove_file(&cfg);
    }
}

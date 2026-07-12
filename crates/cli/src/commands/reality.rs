//! 为 Reality 用户生成真实密钥(xray x25519),写入 spec。
//! 经 RealExecutor 调已安装的 xray 二进制。

use std::path::Path;
use vagent_core::executor::RealExecutor;
use vagent_core::reality::{generate_public_key, generate_short_id};
use vagent_core::{load_spec, save_spec};

/// xray 二进制路径解析优先级:
/// 1. 环境变量 XRAY_BIN(显式覆盖,CI/自定义安装用)
/// 2. PATH 中的 xray(which 可见即返回 "xray")
/// 3. 回退:root 用 /usr/local/bin/xray,普通用户用 ~/.local/bin/xray
pub(crate) fn xray_bin() -> String {
    if let Ok(b) = std::env::var("XRAY_BIN") {
        if !b.is_empty() {
            return b;
        }
    }
    // 尝试 PATH 解析
    if let Ok(out) = std::process::Command::new("sh")
        .arg("-c")
        .arg("command -v xray || true")
        .output()
    {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !p.is_empty() {
            return p;
        }
    }
    // SAFETY: libc::getuid() 是 POSIX 只读 syscall 包装,不触达未初始化内存/无 UB;
    // Rust std 无稳定 uid API,此处用 libc 是惯例做法。
    if unsafe { libc::getuid() } == 0 {
        "/usr/local/bin/xray".to_string()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join("xray")
            .to_string_lossy()
            .to_string()
    }
}

/// 为指定用户(按名字)生成 Reality 公钥 + shortId 并写回 spec。
/// name 为空则对所有 reality 用户批量生成。
pub fn run(config: &Path, name: Option<&str>) -> anyhow::Result<()> {
    let mut spec = match load_spec(config) {
        Ok(s) => s,
        Err(e) => return Err(anyhow::anyhow!("加载配置失败: {e}")),
    };

    let bin = xray_bin();
    let keys = generate_public_key(&bin, &RealExecutor)
        .map_err(|e| anyhow::anyhow!("Reality 密钥生成失败 (xray 路径: {bin}): {e}"))?;
    let sid = generate_short_id();

    let targets: Vec<String> = match name {
        Some(n) => vec![n.to_string()],
        None => spec
            .users
            .iter()
            .filter(|u| u.reality)
            .map(|u| u.name.clone())
            .collect(),
    };
    if targets.is_empty() {
        println!("没有 Reality 用户需要生成密钥");
        return Ok(());
    }

    for t in &targets {
        match spec.users.iter_mut().find(|u| u.name == *t) {
            Some(u) if u.reality => {
                u.reality_pbk = keys.clone();
                u.reality_sid = sid.clone();
                println!("已为 {} 写入 Reality 公钥", t);
            }
            Some(_) => println!("{} 非 Reality 用户,跳过", t),
            None => eprintln!("未找到用户: {}", t),
        }
    }

    save_spec(&spec, config)?;
    println!("Reality 公钥: {}", keys);
    println!("shortId: {}", sid);
    Ok(())
}

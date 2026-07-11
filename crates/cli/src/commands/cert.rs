//! 证书管理:签发 / 续期。经 RealExecutor 调用 acme.sh(真实副作用)。

use std::str::FromStr;
use vagent_core::executor::RealExecutor;
use vagent_core::tls::{self, Ca, Challenge};

/// 签发证书。
/// ca: letsencrypt/zerossl/buypass;dns_hook: Some("dns_cf") 走 DNS,None 走 standalone。
pub fn issue(domain: &str, ca: &str, dns_hook: Option<&str>) -> anyhow::Result<()> {
    let ca = Ca::from_str(ca).map_err(|e| anyhow::anyhow!(e))?;
    let challenge = match dns_hook {
        Some(hook) => Challenge::Dns(hook.to_string()),
        None => Challenge::Standalone,
    };
    tls::issue(domain, ca, &challenge, &RealExecutor).map_err(|e| anyhow::anyhow!(e))?;
    println!("证书签发命令已执行: {domain} ({})", ca.server());
    Ok(())
}

/// 续期所有证书。
pub fn renew() -> anyhow::Result<()> {
    tls::renew(&RealExecutor).map_err(|e| anyhow::anyhow!(e))?;
    println!("证书续期命令已执行");
    Ok(())
}

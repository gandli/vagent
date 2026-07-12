//! TLS 自动化(非 Reality 协议需要)。
//! 拼装 acme.sh 命令 + 续期判断,经 Executor 执行。证书落 /etc/vagent/certs/。
//! 支持多 CA(LetsEncrypt / ZeroSSL / BuyPass)与两种验证模式(standalone / DNS)。

use crate::executor::{Cmd, Executor};
use crate::systemd;
use crate::Error;

/// acme.sh 主目录(root-optional):
/// root 用 /root/.acme.sh(历史约定),非 root 落到 $HOME/.acme.sh,
/// 与项目既有 root-optional 范式(getuid 分支路径)一致。
pub fn acme_home() -> std::path::PathBuf {
    if systemd::is_root() {
        std::path::PathBuf::from("/root/.acme.sh")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".acme.sh")
    }
}

/// 证书颁发机构。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ca {
    LetsEncrypt,
    ZeroSsl,
    BuyPass,
}

impl Ca {
    /// acme.sh --server 参数值。
    pub fn server(&self) -> &'static str {
        match self {
            Ca::LetsEncrypt => "letsencrypt",
            Ca::ZeroSsl => "zerossl",
            Ca::BuyPass => "buypass",
        }
    }

    /// BuyPass 不支持 DNS 申请。
    pub fn supports_dns(&self) -> bool {
        !matches!(self, Ca::BuyPass)
    }
}

impl std::str::FromStr for Ca {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "letsencrypt" | "le" => Ok(Ca::LetsEncrypt),
            "zerossl" | "zero" => Ok(Ca::ZeroSsl),
            "buypass" => Ok(Ca::BuyPass),
            other => Err(format!("未知 CA: {other}")),
        }
    }
}

/// 验证模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Challenge {
    /// standalone HTTP-01(需 80 端口空闲)。
    Standalone,
    /// DNS-01,dns_hook 如 "dns_cf"(Cloudflare)。
    Dns(String),
}

/// 构造签发命令(acme.sh)。cert_dir 跟随 config 父目录(root-optional)。
pub fn issue_cmd(
    domain: &str,
    ca: Ca,
    challenge: &Challenge,
    cert_dir: &str,
) -> Result<Cmd, Error> {
    let mut args: Vec<String> = vec![
        "--issue".into(),
        "-d".into(),
        domain.to_string(),
        "--server".into(),
        ca.server().to_string(),
        "-k".into(),
        "ec-256".into(),
    ];
    match challenge {
        Challenge::Standalone => args.push("--standalone".into()),
        Challenge::Dns(hook) => {
            if !ca.supports_dns() {
                return Err(Error::Unsupported(format!(
                    "{} 不支持 DNS 申请",
                    ca.server()
                )));
            }
            args.push("--dns".into());
            args.push(hook.clone());
        }
    }
    args.push("--home".into());
    args.push(acme_home().to_string_lossy().to_string());
    args.push("--cert-file".into());
    args.push(format!("{cert_dir}/{domain}.cer"));
    args.push("--key-file".into());
    args.push(format!("{cert_dir}/{domain}.key"));
    Ok(Cmd::new("acme.sh").args(args))
}

/// 构造续期(cron)命令。
pub fn renew_cmd() -> Cmd {
    Cmd::new("acme.sh").args(["--cron", "--home", &acme_home().to_string_lossy()])
}

/// 检查证书剩余有效期命令(openssl x509 -enddate)。
pub fn enddate_cmd(domain: &str, cert_dir: &str) -> Cmd {
    Cmd::new("openssl").args([
        "x509",
        "-enddate",
        "-noout",
        "-in",
        &format!("{cert_dir}/{domain}.cer"),
    ])
}

/// 执行签发(经 Executor)。
pub fn issue(
    domain: &str,
    ca: Ca,
    challenge: &Challenge,
    cert_dir: &str,
    ex: &dyn Executor,
) -> Result<(), Error> {
    let out = ex.run(&issue_cmd(domain, ca, challenge, cert_dir)?)?;
    if out.ok() {
        Ok(())
    } else {
        Err(Error::Render(format!(
            "acme.sh issue failed: {}",
            out.stderr
        )))
    }
}

/// 执行续期。
pub fn renew(ex: &dyn Executor) -> Result<(), Error> {
    let out = ex.run(&renew_cmd())?;
    if out.ok() {
        Ok(())
    } else {
        Err(Error::Render(format!(
            "acme.sh renew failed: {}",
            out.stderr
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{ExecOutput, FakeExecutor};
    use std::str::FromStr;

    #[test]
    fn issue_cmd_includes_home_flag() {
        // 非 root 部署:acme.sh 必须显式 --home,否则回退 /root/.acme.sh(Permission denied)
        let c = issue_cmd(
            "v.example.com",
            Ca::LetsEncrypt,
            &Challenge::Standalone,
            "/tmp/certs",
        )
        .unwrap();
        let d = c.display();
        assert!(d.contains("--home"), "签发必须显式指定 acme.sh home: {d}");
    }

    #[test]
    fn renew_cmd_includes_home_flag() {
        let c = renew_cmd();
        let d = c.display();
        assert!(d.contains("--home"), "续期必须显式指定 acme.sh home: {d}");
    }

    #[test]
    fn issue_cmd_standalone_letsencrypt() {
        let c = issue_cmd(
            "v.example.com",
            Ca::LetsEncrypt,
            &Challenge::Standalone,
            "/tmp/certs",
        )
        .unwrap();
        assert_eq!(c.program, "acme.sh");
        let d = c.display();
        assert!(d.contains("-d v.example.com"));
        assert!(d.contains("--server letsencrypt"));
        assert!(d.contains("--standalone"));
        assert!(d.contains("/tmp/certs/v.example.com.cer"));
    }

    #[test]
    fn issue_cmd_dns_zerossl() {
        let c = issue_cmd(
            "x.com",
            Ca::ZeroSsl,
            &Challenge::Dns("dns_cf".into()),
            "/tmp/certs",
        )
        .unwrap();
        let d = c.display();
        assert!(d.contains("--server zerossl"));
        assert!(d.contains("--dns dns_cf"));
    }

    #[test]
    fn buypass_rejects_dns() {
        let r = issue_cmd(
            "x.com",
            Ca::BuyPass,
            &Challenge::Dns("dns_cf".into()),
            "/tmp/certs",
        );
        assert!(r.is_err());
    }

    #[test]
    fn ca_from_str_aliases() {
        assert_eq!(Ca::from_str("le").unwrap(), Ca::LetsEncrypt);
        assert_eq!(Ca::from_str("zerossl").unwrap(), Ca::ZeroSsl);
        assert_eq!(Ca::from_str("buypass").unwrap(), Ca::BuyPass);
        assert!(Ca::from_str("nope").is_err());
    }

    #[test]
    fn ca_dns_support() {
        assert!(Ca::LetsEncrypt.supports_dns());
        assert!(Ca::ZeroSsl.supports_dns());
        assert!(!Ca::BuyPass.supports_dns());
    }

    #[test]
    fn enddate_cmd_targets_cert() {
        let c = enddate_cmd("x.com", "/tmp/certs");
        assert_eq!(c.program, "openssl");
        assert!(c.display().contains("/tmp/certs/x.com.cer"));
    }

    #[test]
    fn issue_failure_propagates() {
        let ex = FakeExecutor::new().expect("acme.sh", ExecOutput::failure(1, "dnserr"));
        assert!(issue(
            "x.com",
            Ca::LetsEncrypt,
            &Challenge::Standalone,
            "/tmp/certs",
            &ex
        )
        .is_err());
    }

    #[test]
    fn issue_success_ok() {
        let ex = FakeExecutor::new().expect("acme.sh", ExecOutput::success("issued"));
        assert!(issue(
            "x.com",
            Ca::LetsEncrypt,
            &Challenge::Standalone,
            "/tmp/certs",
            &ex
        )
        .is_ok());
    }

    #[test]
    fn renew_via_executor() {
        let ex = FakeExecutor::new().expect("acme.sh", ExecOutput::success("renewed"));
        assert!(renew(&ex).is_ok());
    }
}

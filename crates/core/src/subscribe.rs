//! 订阅签发。
//!
//! 单用户链接:`vless://<uuid>@<domain>:<port>?type=tcp&security=reality&pbk=<key>&sid=<sid>#<name>`
//! 多用户 bundle:v2rayN 订阅格式 = Base64(JSON{outbounds:[...]}),每个 Reality 用户一条 outbound。
//! 服务端对 payload 做 HMAC-SHA256 附 `#sig=<hex>`,用于按 id 识别与吊销(客户端不校验)。

use crate::spec::{Protocol, Spec, Transport, User};
use crate::Error;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::path::Path;

type HmacSha256 = Hmac<Sha256>;

/// 传输层 → vless/trojan 链接的 type 参数。
fn link_type(t: &Transport) -> &'static str {
    match t {
        Transport::Tcp => "tcp",
        Transport::Ws => "ws",
        Transport::Grpc => "grpc",
        Transport::Xhttp => "xhttp",
    }
}

/// 生成单用户分享链接(支持 VLESS+Reality / VMess / Trojan / Hysteria2 / Tuic)。
pub fn gen_user(user: &User, spec: &Spec) -> Result<String, Error> {
    let d = &spec.domain;
    let link = match &user.protocol {
        Protocol::Vless if user.reality => {
            let pbk = if user.reality_pbk.is_empty() {
                "<generated-by-xray>"
            } else {
                &user.reality_pbk
            };
            let sid = if user.reality_sid.is_empty() {
                ""
            } else {
                &user.reality_sid
            };
            let query = format!(
                "type={}&security=reality&pbk={}&sid={}&encryption=none&flow=xtls-rprx-vision",
                link_type(&user.transport),
                pbk,
                sid
            );
            format!(
                "vless://{}@{}:{}?{}#{}",
                user.uuid, d, user.port, query, user.name
            )
        }
        Protocol::Vless => format!(
            "vless://{}@{}:{}?type={}#{}",
            user.uuid,
            d,
            user.port,
            link_type(&user.transport),
            user.name
        ),
        Protocol::Vmess => {
            let net = link_type(&user.transport);
            let cfg = serde_json::json!({
                "v": "2", "ps": user.name, "add": d, "port": user.port.to_string(),
                "id": user.uuid, "aid": "0", "net": net, "type": "none",
                "host": d, "path": format!("/{}", user.id), "tls": ""
            });
            let json = serde_json::to_string(&cfg).map_err(|e| Error::Render(e.to_string()))?;
            format!("vmess://{}", B64.encode(json))
        }
        Protocol::Trojan => format!(
            "trojan://{}@{}:{}?security=tls&type={}#{}",
            user.uuid,
            d,
            user.port,
            link_type(&user.transport),
            user.name
        ),
        Protocol::Hysteria2 => format!(
            "hysteria2://{}@{}:{}?sni={}#{}",
            user.uuid, d, user.port, d, user.name
        ),
        Protocol::Tuic => format!(
            "tuic://{}:{}@{}:{}?congestion_control=bbr&alpn=h3&sni={}#{}",
            user.uuid, user.uuid, d, user.port, d, user.name
        ),
        Protocol::Naive => format!(
            "naive+https://{}@{}:{}?sni={}#{}",
            user.uuid, d, user.port, d, user.name
        ),
    };
    Ok(link)
}

/// 多用户 bundle(v2rayN 订阅):Base64(JSON{outbounds})。
pub fn bundle(spec: &Spec) -> Result<String, Error> {
    let outbounds: Vec<serde_json::Value> = spec
        .users
        .iter()
        .filter(|u| matches!(u.protocol, crate::spec::Protocol::Vless) && u.reality)
        .filter(|u| !u.reality_pbk.is_empty()) // 跳过未生成密钥的 reality 用户,避免占位符下发
        .map(|u| {
            serde_json::json!({
                "tag": u.name,
                "type": "vless",
                "server": spec.domain,
                "server_port": u.port,
                "uuid": u.uuid,
                "flow": "xtls-rprx-vision",
                "tls": {
                    "enabled": true,
                    "server_name": spec.domain,
                    "reality": { "enabled": true, "public_key": u.reality_pbk, "short_id": u.reality_sid }
                },
                "transport": "tcp"
            })
        })
        .collect();
    if outbounds.is_empty() {
        return Err(Error::Render(
            "没有可用的 Reality 用户(需先生成 Reality 密钥)".into(),
        ));
    }
    let json = serde_json::to_string(&serde_json::json!({ "outbounds": outbounds }))
        .map_err(|e| Error::Render(e.to_string()))?;
    Ok(B64.encode(json))
}

/// 对链接/bundle 做服务端签名。
pub fn sign(payload: &str, secret: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key len");
    mac.update(payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{payload}#sig={sig}")
}

/// 校验签名是否由本服务端签发。
pub fn verify(link: &str, secret: &[u8]) -> bool {
    let (base, sig) = match link.rsplit_once("#sig=") {
        Some((b, s)) => (b, s),
        None => return false,
    };
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key len");
    mac.update(base.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    expected.len() == sig.len() && expected.bytes().zip(sig.bytes()).all(|(a, b)| a == b)
}

/// 读取或生成 secret(600 权限)。路径跟随 config 父目录(root-optional)。
pub fn load_or_create_secret_at(secret_path: &Path) -> Result<Vec<u8>, Error> {
    if let Ok(s) = std::fs::read(secret_path) {
        return Ok(s);
    }
    let secret: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    if let Some(parent) = secret_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(secret_path, &secret)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(secret_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(secret_path, perms)?;
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::User;

    #[test]
    fn gen_user_formats_vless_link() {
        let mut spec = Spec::default_for("v.example.com");
        let u = User::new(
            "alice",
            crate::spec::Protocol::Vless,
            443,
            true,
            Transport::Tcp,
        );
        spec.add_user("alice", crate::spec::Protocol::Vless, 443, true);
        let link = gen_user(&u, &spec).unwrap();
        assert!(link.starts_with("vless://"));
        assert!(link.contains("v.example.com:443"));
        assert!(link.contains("security=reality"));
        assert!(link.contains("#alice"));
    }

    #[test]
    fn gen_user_vmess_link() {
        let spec = Spec::default_for("v.example.com");
        let u = User::new(
            "bob",
            crate::spec::Protocol::Vmess,
            2053,
            false,
            Transport::Tcp,
        );
        let link = gen_user(&u, &spec).unwrap();
        assert!(link.starts_with("vmess://"));
        let decoded =
            String::from_utf8(B64.decode(link.trim_start_matches("vmess://")).unwrap()).unwrap();
        assert!(decoded.contains("bob"));
        assert!(decoded.contains("2053"));
    }

    #[test]
    fn gen_user_trojan_link() {
        let spec = Spec::default_for("t.example.com");
        let u = User::new(
            "t",
            crate::spec::Protocol::Trojan,
            443,
            false,
            Transport::Tcp,
        );
        let link = gen_user(&u, &spec).unwrap();
        assert!(link.starts_with("trojan://"));
        assert!(link.contains("security=tls"));
    }

    #[test]
    fn gen_user_hysteria2_link() {
        let spec = Spec::default_for("h.example.com");
        let u = User::new(
            "h",
            crate::spec::Protocol::Hysteria2,
            8443,
            false,
            Transport::Tcp,
        );
        let link = gen_user(&u, &spec).unwrap();
        assert!(link.starts_with("hysteria2://"));
        assert!(link.contains("sni=h.example.com"));
    }

    #[test]
    fn gen_user_tuic_link() {
        let spec = Spec::default_for("u.example.com");
        let u = User::new(
            "u",
            crate::spec::Protocol::Tuic,
            9443,
            false,
            Transport::Tcp,
        );
        let link = gen_user(&u, &spec).unwrap();
        assert!(link.starts_with("tuic://"));
        assert!(link.contains("congestion_control=bbr"));
    }

    #[test]
    fn bundle_includes_all_reality_users() {
        let mut spec = Spec::default_for("v.example.com");
        spec.add_user("alice", crate::spec::Protocol::Vless, 443, true);
        spec.add_user("bob", crate::spec::Protocol::Vless, 8443, true);
        spec.add_user("carol", crate::spec::Protocol::Vmess, 9443, false); // 排除:非 reality
                                                                           // Reality 用户须有已生成的公钥才进 bundle(避免下发占位符)
        for u in spec.users.iter_mut() {
            if u.reality {
                u.reality_pbk = "test-public-key".into();
                u.reality_sid = "0123abcd".into();
            }
        }
        let b = bundle(&spec).unwrap();
        let decoded = String::from_utf8(B64.decode(&b).unwrap()).unwrap();
        assert!(decoded.contains("alice"));
        assert!(decoded.contains("bob"));
        assert!(!decoded.contains("carol"));
        assert!(decoded.contains("\"outbounds\""));
        assert!(decoded.contains("test-public-key"));
    }

    #[test]
    fn bundle_errors_when_no_reality_user() {
        // 无可用 Reality 用户(或密钥未生成)时应返回 Err,而非下发占位符
        let mut spec = Spec::default_for("v.example.com");
        spec.add_user("alice", crate::spec::Protocol::Vless, 443, false); // 普通 vless
        spec.add_user("keyless", crate::spec::Protocol::Vless, 8443, true); // reality 但无密钥
        let r = bundle(&spec);
        assert!(r.is_err(), "无可用 Reality 用户时应返回 Err");
    }

    #[test]
    fn load_or_create_secret_at_follows_path() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("vagent").join("secret");
        let s1 = load_or_create_secret_at(&p).unwrap();
        assert_eq!(s1.len(), 32);
        // 二次读取应复用同一 secret
        let s2 = load_or_create_secret_at(&p).unwrap();
        assert_eq!(s1, s2);
        // 文件权限 600
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
    #[test]
    fn sign_and_verify_roundtrip() {
        let secret = b"test-secret-32-bytes-long-1234567890";
        let link = "vless://abc@x.com:443?type=tcp#alice";
        let signed = sign(link, secret);
        assert!(signed.contains("#sig="));
        assert!(verify(&signed, secret));
        assert!(!verify(&signed, b"wrong-secret-wrong-secret-wrong-secre"));
    }

    #[test]
    fn verify_rejects_tampered() {
        let secret = b"test-secret-32-bytes-long-1234567890";
        let signed = sign("vless://abc@x.com:443#alice", secret);
        let tampered = signed.replace("alice", "mallory");
        assert!(!verify(&tampered, secret));
    }
}

//! Xray-core 配置渲染。
//! 产出**完整合法**配置:多协议 inbounds + outbounds(direct/block) + routing。
//! 支持:VLESS+Reality、VMess+WS、Trojan+TLS(Xray 侧协议)。
//! Hysteria2 / Tuic 由 sing-box 渲染(见 render/singbox.rs)。

use crate::spec::{Protocol, Spec, User};
use crate::Error;

/// 单个用户 → Xray inbound(仅 Xray 侧协议;其余返回 None)。
fn inbound_for(u: &User, spec: &Spec) -> Option<serde_json::Value> {
    match u.protocol {
        Protocol::Vless if u.reality => Some(vless_reality(u, spec)),
        Protocol::Vmess => Some(vmess_ws(u)),
        Protocol::Trojan => Some(trojan_tls(u, spec)),
        // VLESS 非 reality、Hysteria2/Tuic/Naive 不在 Xray 侧渲染
        _ => None,
    }
}

fn vless_reality(u: &User, spec: &Spec) -> serde_json::Value {
    serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "vless",
        "settings": {
            "clients": [{ "id": u.uuid, "flow": "xtls-rprx-vision", "level": 0 }],
            "decryption": "none"
        },
        "streamSettings": {
            "network": "tcp",
            "security": "reality",
            "realitySettings": {
                "dest": format!("{}:443", spec.domain),
                "serverNames": [spec.domain.clone()],
                "privateKey": "<generated-by-xray>",
                "shortIds": [""]
            }
        },
        "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
    })
}

fn vmess_ws(u: &User) -> serde_json::Value {
    serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "vmess",
        "settings": {
            "clients": [{ "id": u.uuid, "alterId": 0, "level": 0 }]
        },
        "streamSettings": {
            "network": "ws",
            "wsSettings": { "path": format!("/{}", u.id) }
        },
        "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
    })
}

fn trojan_tls(u: &User, spec: &Spec) -> serde_json::Value {
    serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "trojan",
        "settings": {
            "clients": [{ "password": u.uuid, "level": 0 }]
        },
        "streamSettings": {
            "network": "tcp",
            "security": "tls",
            "tlsSettings": {
                "certificates": [{
                    "certificateFile": format!("/etc/vagent/certs/{}.cer", spec.domain),
                    "keyFile": format!("/etc/vagent/certs/{}.key", spec.domain)
                }]
            }
        },
        "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
    })
}

/// 渲染 Xray-core 配置(JSON)。纯函数:输入 Spec,输出 Value,不触网不落盘。
pub fn render(spec: &Spec) -> Result<serde_json::Value, Error> {
    let inbounds: Vec<serde_json::Value> = spec
        .users
        .iter()
        .filter_map(|u| inbound_for(u, spec))
        .collect();

    let routing = spec.routing_json()?;

    let mut outbounds = vec![
        serde_json::json!({ "protocol": "freedom", "tag": "direct" }),
        serde_json::json!({ "protocol": "blackhole", "tag": "block" }),
    ];
    // 若有域名分流到 WARP,则追加 wireguard 出站(占位密钥,由部署时注入)。
    if spec.needs_warp() {
        outbounds.push(serde_json::json!({
            "protocol": "wireguard",
            "tag": "warp",
            "settings": {
                "secretKey": "<warp-private-key>",
                "address": ["172.16.0.2/32", "fd01:5ca1:ab1e:80fa:ab85:6eea:213f:81a/128"],
                "peers": [{
                    "publicKey": "bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=",
                    "endpoint": "162.159.192.1:2408"
                }]
            }
        }));
    }

    Ok(serde_json::json!({
        "log": { "loglevel": "warning" },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": routing
    }))
}

pub fn render_string(spec: &Spec) -> Result<String, Error> {
    let v = render(spec)?;
    serde_json::to_string_pretty(&v).map_err(|e| Error::Render(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Protocol, Spec, User};

    #[test]
    fn render_filters_non_xray_protocols() {
        let mut spec = Spec::default_for("x.com");
        spec.users.push(User::new("a", Protocol::Vless, 443, true)); // reality → in
        spec.users
            .push(User::new("b", Protocol::Hysteria2, 8443, false)); // sing-box → out
        spec.users.push(User::new("c", Protocol::Tuic, 9443, false)); // sing-box → out
        let v = render(&spec).unwrap();
        let inbounds = v["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1, "仅 Xray 侧协议应入站");
        assert_eq!(inbounds[0]["protocol"], "vless");
    }

    #[test]
    fn render_vmess_ws_inbound() {
        let mut spec = Spec::default_for("x.com");
        spec.users
            .push(User::new("v", Protocol::Vmess, 2053, false));
        let v = render(&spec).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["protocol"], "vmess");
        assert_eq!(ib["streamSettings"]["network"], "ws");
    }

    #[test]
    fn render_trojan_tls_inbound() {
        let mut spec = Spec::default_for("x.com");
        spec.users
            .push(User::new("t", Protocol::Trojan, 443, false));
        let v = render(&spec).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["protocol"], "trojan");
        assert_eq!(ib["streamSettings"]["security"], "tls");
    }

    #[test]
    fn render_has_both_outbounds() {
        let spec = Spec::default_for("x.com");
        let v = render(&spec).unwrap();
        let tags: Vec<&str> = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert!(tags.contains(&"direct"));
        assert!(tags.contains(&"block"));
    }

    #[test]
    fn render_includes_routing_with_rules() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.block_bt = true;
        let v = render(&spec).unwrap();
        assert!(!v["routing"]["rules"].as_array().unwrap().is_empty());
        assert_eq!(v["routing"]["domainStrategy"], "IPIfNonMatch");
    }

    #[test]
    fn render_reality_fields_present() {
        let mut spec = Spec::default_for("x.com");
        spec.users.push(User::new("a", Protocol::Vless, 443, true));
        let v = render(&spec).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["streamSettings"]["security"], "reality");
        assert_eq!(ib["streamSettings"]["realitySettings"]["dest"], "x.com:443");
    }

    #[test]
    fn render_adds_warp_outbound_when_needed() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.warp_domains.push("netflix.com".into());
        let v = render(&spec).unwrap();
        let tags: Vec<&str> = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert!(tags.contains(&"warp"));
        let warp = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "warp")
            .unwrap();
        assert_eq!(warp["protocol"], "wireguard");
    }

    #[test]
    fn render_no_warp_outbound_by_default() {
        let spec = Spec::default_for("x.com");
        let v = render(&spec).unwrap();
        let tags: Vec<&str> = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert!(!tags.contains(&"warp"));
    }
}

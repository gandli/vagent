//! Xray-core 配置渲染。
//! 产出**完整合法**配置:多协议 inbounds + outbounds(direct/block) + routing。
//! 支持:VLESS+Reality、VMess+WS、Trojan+TLS(Xray 侧协议)。
//! Hysteria2 / Tuic 由 sing-box 渲染(见 render/singbox.rs)。

use crate::spec::{Protocol, Spec, Transport, User};
use crate::Error;
use std::path::Path;

/// 单个用户 → Xray inbound(仅 Xray 侧协议;其余返回 None)。
/// 返回 Result:reality 用户缺密钥时 Err(占位符改报错)。
fn inbound_for(
    u: &User,
    spec: &Spec,
    cert_cer: &str,
    cert_key: &str,
) -> Result<Option<serde_json::Value>, Error> {
    match (&u.protocol, u.reality, &u.transport) {
        (Protocol::Vless, true, transport) => Ok(Some(vless_reality(u, spec, transport)?)),
        (Protocol::Vless, false, transport) => Ok(Some(vless_plain(u, transport))),
        (Protocol::Vmess, _, _) => Ok(Some(vmess_ws(u))),
        (Protocol::Trojan, _, transport) => Ok(Some(trojan_tls(u, transport, cert_cer, cert_key))),
        // Hysteria2/Tuic/Naive 不在 Xray 侧渲染
        _ => Ok(None),
    }
}

/// 生成 streamSettings(按传输层)。
fn stream_for(transport: &Transport) -> serde_json::Value {
    match transport {
        Transport::Tcp => serde_json::json!({ "network": "tcp" }),
        Transport::Ws => serde_json::json!({ "network": "ws", "wsSettings": { "path": "/ws" } }),
        Transport::Grpc => serde_json::json!({
            "network": "grpc",
            "grpcSettings": { "serviceName": "vagent" }
        }),
        Transport::Xhttp => serde_json::json!({
            "network": "xhttp",
            "xhttpSettings": { "path": "/xhttp" }
        }),
    }
}

fn vless_reality(u: &User, spec: &Spec, transport: &Transport) -> Result<serde_json::Value, Error> {
    // 单一真相源:reality 用户必须有真实公钥,缺失即 Err(不再内联检查)
    let (pbk, sid) = u.require_reality_keys()?;
    let sid = if sid.is_empty() {
        String::new()
    } else {
        sid.to_string()
    };
    let mut stream = stream_for(transport);
    stream["security"] = serde_json::json!("reality");
    stream["realitySettings"] = serde_json::json!({
        "dest": format!("{}:443", spec.domain),
        "serverNames": [spec.domain.clone()],
        "privateKey": pbk,
        "shortIds": [sid]
    });
    Ok(serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "vless",
        "settings": {
            "clients": [{ "id": u.uuid, "flow": "xtls-rprx-vision", "level": 0 }],
            "decryption": "none"
        },
        "streamSettings": stream,
        "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
    }))
}

fn vless_plain(u: &User, transport: &Transport) -> serde_json::Value {
    serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "vless",
        "settings": {
            "clients": [{ "id": u.uuid, "level": 0 }],
            "decryption": "none"
        },
        "streamSettings": stream_for(transport),
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

fn trojan_tls(
    u: &User,
    transport: &Transport,
    cert_cer: &str,
    cert_key: &str,
) -> serde_json::Value {
    let mut stream = stream_for(transport);
    stream["security"] = serde_json::json!("tls");
    stream["tlsSettings"] = serde_json::json!({
        "certificates": [{
            "certificateFile": cert_cer,
            "keyFile": cert_key
        }]
    });
    serde_json::json!({
        "listen": "0.0.0.0",
        "port": u.port,
        "protocol": "trojan",
        "settings": {
            "clients": [{ "password": u.uuid, "level": 0 }]
        },
        "streamSettings": stream,
        "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
    })
}

/// 渲染 Xray-core 配置(JSON)。纯函数:输入 Spec,输出 Value,不触网不落盘。
pub fn render(spec: &Spec, base_dir: &Path) -> Result<serde_json::Value, Error> {
    let cert_cer = base_dir
        .join("certs")
        .join(format!("{}.cer", spec.domain))
        .to_string_lossy()
        .to_string();
    let cert_key = base_dir
        .join("certs")
        .join(format!("{}.key", spec.domain))
        .to_string_lossy()
        .to_string();
    let inbounds: Vec<serde_json::Value> = spec
        .users
        .iter()
        .map(|u| inbound_for(u, spec, &cert_cer, &cert_key))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
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
    // 高级用户自定义出站(原样拼入,可接入第三方机场节点/任意自定义)。
    // JSON 非法则直接 Err,不静默吞错(m06)。
    for raw in &spec.rules.custom_outbounds {
        let v: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| Error::Render(format!("custom_outbounds 非法 JSON: {e} @ {raw}")))?;
        outbounds.push(v);
    }

    Ok(serde_json::json!({
        "log": { "loglevel": "warning" },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": routing
    }))
}

pub fn render_string(spec: &Spec, base_dir: &Path) -> Result<String, Error> {
    let v = render(spec, base_dir)?;
    serde_json::to_string_pretty(&v).map_err(|e| Error::Render(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Protocol, Spec, Transport, User};

    #[test]
    fn render_filters_non_xray_protocols() {
        let mut spec = Spec::default_for("x.com");
        {
            let mut a = User::new("a", Protocol::Vless, 443, true, Transport::Tcp);
            a.reality_pbk = "abc123pubkey".to_string();
            a.reality_sid = "abcd1234".to_string();
            spec.users.push(a); // reality → in
        }
        spec.users.push(User::new(
            "b",
            Protocol::Hysteria2,
            8443,
            false,
            Transport::Tcp,
        )); // sing-box → out
        spec.users
            .push(User::new("c", Protocol::Tuic, 9443, false, Transport::Tcp)); // sing-box → out
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let inbounds = v["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1, "仅 Xray 侧协议应入站");
        assert_eq!(inbounds[0]["protocol"], "vless");
    }

    #[test]
    fn render_vmess_ws_inbound() {
        let mut spec = Spec::default_for("x.com");
        spec.users
            .push(User::new("v", Protocol::Vmess, 2053, false, Transport::Tcp));
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["protocol"], "vmess");
        assert_eq!(ib["streamSettings"]["network"], "ws");
    }

    #[test]
    fn render_trojan_tls_inbound() {
        let mut spec = Spec::default_for("x.com");
        spec.users
            .push(User::new("t", Protocol::Trojan, 443, false, Transport::Tcp));
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["protocol"], "trojan");
        assert_eq!(ib["streamSettings"]["security"], "tls");
    }

    #[test]
    fn render_has_both_outbounds() {
        let spec = Spec::default_for("x.com");
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
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
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        assert!(!v["routing"]["rules"].as_array().unwrap().is_empty());
        assert_eq!(v["routing"]["domainStrategy"], "IPIfNonMatch");
    }

    #[test]
    fn custom_outbounds_injected_xray() {
        // 接入第三方机场节点:把机场出站 JSON 原样拼入 outbounds
        let mut spec = Spec::default_for("x.com");
        let airport = serde_json::json!({
            "protocol": "vless",
            "tag": "airport",
            "settings": { "vnext": [{
                "address": "hk.airport.com",
                "port": 443,
                "users": [{"id": "uuid-here"}]
            }] }
        });
        spec.rules
            .custom_outbounds
            .push(serde_json::to_string(&airport).unwrap());
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let tags: Vec<&str> = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert!(
            tags.contains(&"airport"),
            "custom_outbounds 应原样注入: {tags:?}"
        );
    }

    #[test]
    fn custom_outbounds_invalid_json_errors() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.custom_outbounds.push("not json".into());
        assert!(
            render(&spec, Path::new("/etc/vagent/spec.toml")).is_err(),
            "非法 JSON 应 Err 而非静默"
        );
    }

    #[test]
    fn render_reality_fields_present() {
        let mut spec = Spec::default_for("x.com");
        {
            let mut a = User::new("a", Protocol::Vless, 443, true, Transport::Tcp);
            a.reality_pbk = "abc123pubkey".to_string();
            a.reality_sid = "abcd1234".to_string();
            spec.users.push(a);
        }
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let ib = &v["inbounds"][0];
        assert_eq!(ib["streamSettings"]["security"], "reality");
        assert_eq!(ib["streamSettings"]["realitySettings"]["dest"], "x.com:443");
    }

    #[test]
    fn render_adds_warp_outbound_when_needed() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.warp_domains.push("netflix.com".into());
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
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
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let tags: Vec<&str> = v["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert!(!tags.contains(&"warp"));
    }

    #[test]
    fn render_reality_without_pbk_errors() {
        // R5 闭环:reality 用户缺公钥不得发射 <generated-by-xray> 占位符,应报错
        let mut spec = Spec::default_for("x.com");
        spec.users
            .push(User::new("a", Protocol::Vless, 443, true, Transport::Tcp));
        let r = render(&spec, Path::new("/etc/vagent/spec.toml"));
        assert!(r.is_err(), "缺密钥的 reality 用户渲染应失败");
        let msg = format!("{}", r.unwrap_err());
        assert!(!msg.contains("<generated-by-xray>"), "不应含占位符: {msg}");
    }
}

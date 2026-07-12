//! sing-box 配置渲染。
//! 负责 Xray 不便承载的现代协议:Hysteria2、Tuic。
//! 其余协议由 Xray 渲染(见 render/xray.rs)。

use crate::spec::{Protocol, Spec, User};
use crate::Error;
use serde_json::json;
use std::path::Path;

/// 单个用户 → sing-box inbound(Hysteria2 / Tuic)。
fn inbound_for(u: &User, cert_cer: &str, cert_key: &str) -> Option<serde_json::Value> {
    match (&u.protocol, &u.transport) {
        (Protocol::Hysteria2, _) => Some(hysteria2(u, cert_cer, cert_key)),
        (Protocol::Tuic, _) => Some(tuic(u, cert_cer, cert_key)),
        _ => None,
    }
}

fn hysteria2(u: &User, cert_cer: &str, cert_key: &str) -> serde_json::Value {
    json!({
        "type": "hysteria2",
        "tag": format!("hy2-{}", u.id),
        "listen": "::",
        "listen_port": u.port,
        "users": [{ "password": u.uuid }],
        "tls": {
            "enabled": true,
            "certificate_path": cert_cer,
            "key_path": cert_key
        }
    })
}

fn tuic(u: &User, cert_cer: &str, cert_key: &str) -> serde_json::Value {
    json!({
        "type": "tuic",
        "tag": format!("tuic-{}", u.id),
        "listen": "::",
        "listen_port": u.port,
        "users": [{ "uuid": u.uuid, "password": u.uuid }],
        "congestion_control": "bbr",
        "tls": {
            "enabled": true,
            "alpn": ["h3"],
            "certificate_path": cert_cer,
            "key_path": cert_key
        }
    })
}

/// 渲染 sing-box 配置(JSON)。纯函数。
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
        .filter_map(|u| inbound_for(u, &cert_cer, &cert_key))
        .collect();

    let mut outbounds = vec![
        json!({ "type": "direct", "tag": "direct" }),
        json!({ "type": "block", "tag": "block" }),
    ];
    // 高级用户自定义出站(原样拼入,可接入第三方机场节点/任意自定义)。
    // JSON 非法则直接 Err,不静默吞错(m06)。
    for raw in &spec.rules.custom_outbounds {
        let v: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| Error::Render(format!("custom_outbounds 非法 JSON: {e} @ {raw}")))?;
        outbounds.push(v);
    }

    Ok(json!({
        "log": { "level": "warn" },
        "inbounds": inbounds,
        "outbounds": outbounds
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
    use std::path::Path;

    #[test]
    fn renders_hy2_and_tuic_inbounds() {
        let mut spec = Spec::default_for("x.com");
        spec.users.push(User::new(
            "h",
            Protocol::Hysteria2,
            8443,
            false,
            Transport::Tcp,
        ));
        spec.users
            .push(User::new("t", Protocol::Tuic, 9443, false, Transport::Tcp));
        let v = render(&spec, Path::new("/etc/vagent/spec.toml")).unwrap();
        let ib = v["inbounds"].as_array().unwrap();
        let tags: Vec<&str> = ib.iter().map(|x| x["tag"].as_str().unwrap()).collect();
        // 约定 tag = "{proto}-{id}",id 由 new 生成(非 name);只校验前缀存在
        assert!(
            tags.iter().any(|t| t.starts_with("hy2-")),
            "应有 hy2- inbound: {tags:?}"
        );
        assert!(
            tags.iter().any(|t| t.starts_with("tuic-")),
            "应有 tuic- inbound: {tags:?}"
        );
    }

    #[test]
    fn custom_outbounds_injected_singbox() {
        // 接入第三方机场节点:把机场出站 JSON 原样拼入 outbounds
        let mut spec = Spec::default_for("x.com");
        spec.rules.custom_outbounds.push(
            "{\"type\":\"vless\",\"tag\":\"airport\",\"server\":\"hk.airport.com\",\"server_port\":443,\"uuid\":\"uuid-here\"}".to_string(),
        );
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
}

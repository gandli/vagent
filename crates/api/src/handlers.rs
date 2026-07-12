//! API 视图层:纯函数,将 Spec 转成 JSON 视图,可单测、不碰网络。

use serde_json::json;
use vagent_core::Spec;

/// 鉴权判定(纯函数,便于单测)。
/// - is_write: 是否为写操作(POST/PUT/DELETE)。
/// - configured_token: 服务端配置的 token(来自 VAGENT_API_TOKEN),None 表示未配置。
/// - auth_header: 请求的 Authorization 头值。
///
/// 规则:
/// - 配置了 token:任何请求都须 `Authorization: Bearer <token>` 且匹配,否则拒绝。
/// - 未配置 token:只读放行(loopback 面板可用);写操作一律拒绝(不允许无凭证修改)。
pub fn is_authorized(
    is_write: bool,
    configured_token: Option<&str>,
    auth_header: Option<&str>,
) -> bool {
    match configured_token {
        Some(token) if !token.is_empty() => {
            let expected = format!("Bearer {token}");
            auth_header == Some(expected.as_str())
        }
        _ => {
            // 未配置 token:只读放行,写操作拒绝
            !is_write
        }
    }
}

/// 状态视图(供 /api/status 与面板渲染)。
pub fn status_view(spec: &Spec) -> serde_json::Value {
    json!({
        "domain": spec.domain,
        "cores": {
            "xray": spec.cores.xray,
            "singbox": spec.cores.singbox
        },
        "rules": {
            "domain_blocklist": spec.rules.domain_blocklist,
            "block_bt": spec.rules.block_bt
        },
        "users": spec.users.iter().map(|u| json!({
            "name": u.name,
            "protocol": format!("{:?}", u.protocol).to_lowercase(),
            "port": u.port,
            "reality": u.reality
        })).collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vagent_core::Protocol;

    #[test]
    fn status_view_includes_domain_and_users() {
        let mut spec = Spec::default_for("v.example.com");
        spec.add_user("alice", Protocol::Vless, 443, true);
        let v = status_view(&spec);
        assert_eq!(v["domain"], serde_json::json!("v.example.com"));
        assert_eq!(v["users"].as_array().unwrap().len(), 1);
        assert_eq!(v["users"][0]["name"], serde_json::json!("alice"));
        assert_eq!(v["users"][0]["protocol"], serde_json::json!("vless"));
        assert_eq!(v["users"][0]["reality"], serde_json::json!(true));
    }

    #[test]
    fn status_view_rules_present() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.block_bt = true;
        let v = status_view(&spec);
        assert_eq!(v["rules"]["block_bt"], serde_json::json!(true));
    }

    #[test]
    fn auth_with_token_requires_matching_bearer() {
        let tok = Some("s3cret");
        assert!(is_authorized(true, tok, Some("Bearer s3cret")));
        assert!(is_authorized(false, tok, Some("Bearer s3cret")));
        assert!(!is_authorized(true, tok, Some("Bearer wrong")));
        assert!(!is_authorized(true, tok, None));
        assert!(!is_authorized(false, tok, Some("s3cret"))); // 缺 Bearer 前缀
    }

    #[test]
    fn auth_without_token_allows_read_denies_write() {
        assert!(is_authorized(false, None, None), "只读应放行");
        assert!(!is_authorized(true, None, None), "写操作无 token 应拒绝");
        assert!(
            !is_authorized(true, Some(""), None),
            "空 token 视为未配置,写拒绝"
        );
        assert!(
            is_authorized(false, Some(""), None),
            "空 token 视为未配置,读放行"
        );
    }
}

//! 分流规则:MVP 仅「黑名单域名」+「BT 阻断」,spec 驱动 → 渲染成内核 routing 段。
//! 与 render 同构:纯函数,不碰系统,可单测。

use crate::spec::Spec;
use crate::Error;
use serde_json::json;

impl Spec {
    /// 渲染 routing 规则段(Xray routing 结构)。
    /// 规则顺序即优先级(Xray 取首个匹配):
    /// 直连白名单 → 广告拦截 → 域名黑名单 → BT 阻断 → WARP 分流。
    pub fn routing_json(&self) -> Result<serde_json::Value, Error> {
        let mut rules: Vec<serde_json::Value> = vec![];

        // 1. 强制直连白名单(优先级最高)
        if !self.rules.direct_domains.is_empty() {
            rules.push(json!({
                "type": "field",
                "domain": self.rules.direct_domains,
                "outboundTag": "direct"
            }));
        }
        // 2. 广告拦截(geosite)
        if self.rules.block_ads {
            rules.push(json!({
                "type": "field",
                "domain": ["geosite:category-ads-all"],
                "outboundTag": "block"
            }));
        }
        // 3. 域名黑名单
        if !self.rules.domain_blocklist.is_empty() {
            rules.push(json!({
                "type": "field",
                "domain": self.rules.domain_blocklist,
                "outboundTag": "block"
            }));
        }
        // 4. BT 阻断
        if self.rules.block_bt {
            rules.push(json!({
                "type": "field",
                "protocol": ["bittorrent"],
                "outboundTag": "block"
            }));
        }
        // 5. WARP 分流(指定域名走 WARP 出站)
        if !self.rules.warp_domains.is_empty() {
            rules.push(json!({
                "type": "field",
                "domain": self.rules.warp_domains,
                "outboundTag": "warp"
            }));
        }
        // 6. 高级用户自定义规则(原样拼入,流量兜底前最后生效)。
        // JSON 非法则直接 Err,不静默吞错(m06)。
        for raw in &self.rules.extra_routing_rules {
            let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
                Error::Render(format!("extra_routing_rules 非法 JSON: {e} @ {raw}"))
            })?;
            rules.push(v);
        }

        Ok(json!({
            "domainStrategy": "IPIfNonMatch",
            "rules": rules
        }))
    }

    /// 是否需要 WARP 出站(有域名分流到 WARP 时)。
    pub fn needs_warp(&self) -> bool {
        !self.rules.warp_domains.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Spec;

    #[test]
    fn empty_rules_no_block_entries() {
        let spec = Spec::default_for("x.com");
        let r = spec.routing_json().unwrap();
        assert_eq!(r["rules"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn domain_blocklist_renders_field() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.domain_blocklist.push("evil.com".into());
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["outboundTag"], "block");
        assert!(rules[0]["domain"]
            .as_array()
            .unwrap()
            .contains(&json!("evil.com")));
    }

    #[test]
    fn block_bt_renders_protocol_rule() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.block_bt = true;
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["protocol"][0], "bittorrent");
    }

    #[test]
    fn both_rules_combine() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.domain_blocklist.push("a.com".into());
        spec.rules.block_bt = true;
        let r = spec.routing_json().unwrap();
        assert_eq!(r["rules"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn direct_domains_render_first() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.direct_domains.push("bank.com".into());
        spec.rules.block_ads = true;
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        // 白名单直连必须排在广告拦截之前
        assert_eq!(rules[0]["outboundTag"], "direct");
        assert_eq!(rules[1]["outboundTag"], "block");
    }

    #[test]
    fn block_ads_uses_geosite() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.block_ads = true;
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        assert_eq!(rules[0]["domain"][0], "geosite:category-ads-all");
    }

    #[test]
    fn warp_domains_route_to_warp() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.warp_domains.push("netflix.com".into());
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        assert_eq!(rules[0]["outboundTag"], "warp");
        assert!(spec.needs_warp());
    }

    #[test]
    fn extra_routing_rules_injected() {
        let mut spec = Spec::default_for("x.com");
        spec.rules
            .extra_routing_rules
            .push(r#"{"type":"field","ipinfo_country":"cn","outboundTag":"direct"}"#.into());
        let r = spec.routing_json().unwrap();
        let rules = r["rules"].as_array().unwrap();
        // 自定义规则应出现在结构化规则之后
        assert!(
            rules.iter().any(|x| x["ipinfo_country"] == "cn"),
            "extra_routing_rules 应原样注入: {rules:?}"
        );
    }

    #[test]
    fn extra_routing_rules_invalid_json_errors() {
        let mut spec = Spec::default_for("x.com");
        spec.rules.extra_routing_rules.push("not json".into());
        assert!(spec.routing_json().is_err(), "非法 JSON 应 Err 而非静默");
    }
}

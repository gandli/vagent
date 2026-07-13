use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Error;

/// 分流规则:黑名单域名 + BT 阻断 + 广告拦截 + WARP/直连分流。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Rules {
    /// 禁止访问的域名(黑名单)。
    #[serde(default)]
    pub domain_blocklist: Vec<String>,
    /// 是否阻断 P2P / BT 下载。
    #[serde(default)]
    pub block_bt: bool,
    /// 是否用 geosite 拦截广告(category-ads-all)。
    #[serde(default)]
    pub block_ads: bool,
    /// 走 WARP 出站的域名(解锁流媒体/规避 IP 验证)。
    #[serde(default)]
    pub warp_domains: Vec<String>,
    /// 强制直连的域名(白名单,优先级最高)。
    #[serde(default)]
    pub direct_domains: Vec<String>,
    /// 高级用户自定义分流规则(原样拼入 xray/sing-box routing.rules)。
    /// 每行一条规则 JSON,如 {"type":"field","ipinfo_country":"cn","outboundTag":"direct"}。
    /// 用于接入 geoip / 第三方规则 / 任意自定义,无需改 core 代码。空则无影响。
    #[serde(default)]
    pub extra_routing_rules: Vec<String>,
    /// 高级用户自定义出站(原样拼入 outbounds)。
    /// 每行一段出站 JSON,可接入第三方机场节点 / 任意自定义出站。
    /// vagent 只做 JSON 合法性校验,不解析语义(用户自担其责)。空则无影响。
    #[serde(default)]
    pub custom_outbounds: Vec<String>,
}

impl Rules {
    pub fn empty() -> Self {
        Rules::default()
    }
}

/// nginx 前端管理(占 443 反代本机 + 可选伪装站 SNI)。
/// root VPS 标准路径:nginx 以 root 持有 443,xray/sing-box 绑高位端口(8443),
/// 由 nginx 反代进来。非 root 部署可留空(直接用高位端口,无需 nginx)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NginxConfig {
    /// 入站反代:nginx 监听 443,转发到 127.0.0.1:<reverse_port>。
    /// 让 xray/sing-box 对外暴露标准 443(否则需绑高位端口)。
    #[serde(default)]
    pub reverse_proxy: bool,
    /// 反代目标端口(本机 xray/sing-box 监听端口,通常 8443)。
    #[serde(default = "default_reverse_port")]
    pub reverse_port: u16,
    /// 伪装站 SNI 反代:把流量透传到外部真实站点(domain:443),用于 Reality 流量特征伪装。
    #[serde(default)]
    pub sni_proxy: bool,
}

fn default_reverse_port() -> u16 {
    8443
}

impl NginxConfig {
    pub fn empty() -> Self {
        NginxConfig::default()
    }
    /// 是否需要在渲染时生成任何 nginx 配置。
    pub fn active(&self) -> bool {
        self.reverse_proxy || self.sni_proxy
    }
}

/// 声明式部署规格 —— 整个系统的唯一真相来源。
/// 所有渲染、状态、订阅都从 Spec 推导,不反推 JSON。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spec {
    pub version: u8,
    pub domain: String,
    #[serde(default)]
    pub cores: Cores,
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub rules: Rules,
    /// nginx 前端(占 443 反代本机 + 伪装站)。默认空 = 不生成 nginx 配置。
    #[serde(default)]
    pub nginx: NginxConfig,
    /// 端口跳跃(对标 v2ray-agent dokodemo-door):开启后真实 inbound 监听 127.0.0.1,
    /// 由 dokodemo-door 在跳跃端口(范围首端口)接收外部流量并转发。抗封锁/规避单端口限流。
    #[serde(default)]
    pub port_hopping: Option<PortHopping>,
}

/// 端口跳跃范围。start..=end 为防火墙开放段;渲染时 dokodemo-door 监听 start 端口转发到真实 inbound。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortHopping {
    pub start: u16,
    pub end: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cores {
    #[serde(default = "default_true")]
    pub xray: bool,
    #[serde(default)]
    pub singbox: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub port: u16,
    #[serde(default)]
    pub reality: bool,
    pub uuid: String,
    /// Reality 公钥(客户端用)。由 xray x25519 生成,留空则渲染时占位。
    #[serde(default)]
    pub reality_pbk: String,
    /// Reality shortId(客户端用,可选)。
    #[serde(default)]
    pub reality_sid: String,
    /// 传输层(默认 tcp)。
    #[serde(default)]
    pub transport: Transport,
}

impl User {
    /// reality 用户必须有真实公钥,缺失即 Err(单一真相源,
    /// 避免 <generated-by-xray> 占位符下发)。sid 可空(reality shortId 可选),
    /// 此时返回空串。
    pub fn require_reality_keys(&self) -> Result<(&str, &str), Error> {
        if self.reality_pbk.is_empty() {
            return Err(Error::Render(format!(
                "用户 {} 是 Reality 用户但未生成密钥(reality_pbk 为空),无法生成有效链接/配置",
                self.name
            )));
        }
        // sid 可空:reality shortId 可选,空串视为未设置
        Ok((self.reality_pbk.as_str(), self.reality_sid.as_str()))
    }

    /// 是否为可渲染的 reality 用户(有公钥)。供 bundle 等过滤复用。
    pub fn is_renderable_reality(&self) -> bool {
        matches!(self.protocol, Protocol::Vless)
            && self.reality
            && self.require_reality_keys().is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vless,
    Vmess,
    Trojan,
    Hysteria2,
    Tuic,
    Naive,
    AnyTls,
}

/// 传输层(决定 streamSettings.network / grpc / xhttp)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    #[default]
    Tcp,
    Ws,
    Grpc,
    Xhttp,
    HttpUpgrade,
}

impl std::str::FromStr for Transport {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Transport::Tcp),
            "ws" | "websocket" => Ok(Transport::Ws),
            "grpc" => Ok(Transport::Grpc),
            "xhttp" => Ok(Transport::Xhttp),
            "httpupgrade" | "h2" => Ok(Transport::HttpUpgrade),
            other => Err(format!("未知传输: {other}")),
        }
    }
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Transport::Tcp => "tcp",
            Transport::Ws => "ws",
            Transport::Grpc => "grpc",
            Transport::Xhttp => "xhttp",
            Transport::HttpUpgrade => "httpupgrade",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for Protocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "vless" => Ok(Protocol::Vless),
            "vmess" => Ok(Protocol::Vmess),
            "trojan" => Ok(Protocol::Trojan),
            "hysteria2" | "hy2" => Ok(Protocol::Hysteria2),
            "tuic" => Ok(Protocol::Tuic),
            "naive" => Ok(Protocol::Naive),
            "anytls" => Ok(Protocol::AnyTls),
            other => Err(format!("未知协议: {other}")),
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Protocol::Vless => "vless",
            Protocol::Vmess => "vmess",
            Protocol::Trojan => "trojan",
            Protocol::Hysteria2 => "hysteria2",
            Protocol::Tuic => "tuic",
            Protocol::Naive => "naive",
            Protocol::AnyTls => "anytls",
        };
        f.write_str(s)
    }
}

impl Default for Cores {
    fn default() -> Self {
        Cores {
            xray: true,
            singbox: false,
        }
    }
}

impl Spec {
    /// 生成一个最小默认 Spec(`vagent init` 用)。
    pub fn default_for(domain: &str) -> Self {
        Spec {
            version: 1,
            domain: domain.to_string(),
            cores: Cores::default(),
            users: vec![],
            rules: Rules::empty(),
            nginx: NginxConfig::empty(),
            port_hopping: None,
        }
    }

    /// 按选定的协议组合生成 Spec(安装时多选协议)。
    /// 每个协议加 1 个默认用户,并开启对应内核:
    /// Vless / Vmess / Trojan 走 xray,Hysteria2 / Tuic / Naive 走 sing-box。
    /// Reality 用户标记 reality=true(pbk 需后续生成密钥)。
    pub fn default_for_protocols(domain: &str, protocols: &[Protocol]) -> Self {
        let mut spec = Spec::default_for(domain);
        for p in protocols {
            match p {
                Protocol::Vless => {
                    spec.cores.xray = true;
                    spec.add_user("vless", Protocol::Vless, 443, false);
                }
                Protocol::Vmess => {
                    spec.cores.xray = true;
                    spec.add_user("vmess", Protocol::Vmess, 2053, false);
                }
                Protocol::Trojan => {
                    spec.cores.xray = true;
                    spec.add_user("trojan", Protocol::Trojan, 443, false);
                }
                Protocol::Hysteria2 => {
                    spec.cores.singbox = true;
                    spec.add_user("hysteria2", Protocol::Hysteria2, 8443, false);
                }
                Protocol::Tuic => {
                    spec.cores.singbox = true;
                    spec.add_user("tuic", Protocol::Tuic, 9443, false);
                }
                Protocol::Naive => {
                    spec.cores.singbox = true;
                    spec.add_user("naive", Protocol::Naive, 8448, false);
                }
                Protocol::AnyTls => {
                    spec.cores.singbox = true;
                    spec.add_user("anytls", Protocol::AnyTls, 8443, false);
                }
            }
        }
        spec
    }

    /// 配置文件的默认路径:root 用 /etc/vagent/spec.toml,普通用户用 ~/.config/vagent/spec.toml。
    pub fn default_config_path() -> std::path::PathBuf {
        if let Ok(uid) = std::env::var("UID") {
            if uid == "0" {
                return std::path::PathBuf::from("/etc/vagent/spec.toml");
            }
        // SAFETY: libc::getuid() 是 POSIX 只读 syscall 包装,不触达未初始化内存/无 UB;
        // Rust std 无稳定 uid API,此处用 libc 是惯例做法。
        } else if unsafe { libc::getuid() } == 0 {
            return std::path::PathBuf::from("/etc/vagent/spec.toml");
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("vagent")
            .join("spec.toml")
    }

    /// 派生目录锚点:配置文件的父目录。所有 cores/certs/扫描 路径都从它推导,
    /// 不硬编码 /etc,从而支持普通用户安装。
    pub fn base_dir(config: &std::path::Path) -> std::path::PathBuf {
        config
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// 新增一个用户,自动生成 id / uuid。
    pub fn add_user(&mut self, name: &str, protocol: Protocol, port: u16, reality: bool) {
        self.users
            .push(User::new(name, protocol, port, reality, Transport::Tcp));
    }

    /// 按名字删除用户,返回删除的数量。
    pub fn remove_user(&mut self, name: &str) -> usize {
        let before = self.users.len();
        self.users.retain(|u| u.name != name);
        before - self.users.len()
    }

    /// 是否存在需要 sing-box 承载的协议(Hysteria2 / Tuic)。
    pub fn needs_singbox(&self) -> bool {
        self.users
            .iter()
            .any(|u| matches!(u.protocol, Protocol::Hysteria2 | Protocol::Tuic))
    }

    /// 是否存在需要 Xray 承载的协议。
    pub fn needs_xray(&self) -> bool {
        self.users.iter().any(|u| {
            matches!(
                u.protocol,
                Protocol::Vless | Protocol::Vmess | Protocol::Trojan
            )
        })
    }
}

impl User {
    pub fn new(
        name: &str,
        protocol: Protocol,
        port: u16,
        reality: bool,
        transport: Transport,
    ) -> Self {
        User {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            protocol,
            port,
            reality,
            uuid: Uuid::new_v4().to_string(),
            reality_pbk: String::new(),
            reality_sid: String::new(),
            transport,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_for_protocols_enables_cores_and_users() {
        let protos = [Protocol::Vless, Protocol::Hysteria2, Protocol::Tuic];
        let s = Spec::default_for_protocols("x.com", &protos);
        // xray (vless) + singbox (hy2/tuic) 都应开启
        assert!(s.cores.xray);
        assert!(s.cores.singbox);
        // 每协议 1 个用户
        assert_eq!(s.users.len(), 3);
        // 协议正确
        assert!(s.users.iter().any(|u| u.protocol == Protocol::Vless));
        assert!(s.users.iter().any(|u| u.protocol == Protocol::Hysteria2));
        assert!(s.users.iter().any(|u| u.protocol == Protocol::Tuic));
        // 端口正确(hy2=8443, tuic=9443)
        assert!(s
            .users
            .iter()
            .any(|u| u.protocol == Protocol::Hysteria2 && u.port == 8443));
        assert!(s
            .users
            .iter()
            .any(|u| u.protocol == Protocol::Tuic && u.port == 9443));
    }

    #[test]
    fn default_for_protocols_empty_is_minimal() {
        let s = Spec::default_for_protocols("x.com", &[]);
        assert!(s.cores.xray); // default_for 仍开 xray
        assert!(!s.cores.singbox);
        assert_eq!(s.users.len(), 0);
    }

    #[test]
    fn user_new_generates_unique_ids() {
        let a = User::new("alice", Protocol::Vless, 443, true, Transport::Tcp);
        let b = User::new("bob", Protocol::Vless, 443, true, Transport::Tcp);
        assert!(!a.uuid.is_empty());
        assert!(!b.uuid.is_empty());
        assert_ne!(a.id, b.id);
        assert_ne!(a.uuid, b.uuid);
    }

    #[test]
    fn add_user_appends() {
        let mut s = Spec::default_for("x.com");
        s.add_user("alice", Protocol::Vless, 443, true);
        assert_eq!(s.users.len(), 1);
        assert_eq!(s.users[0].name, "alice");
    }

    #[test]
    fn protocol_serde_roundtrip() {
        let toml = r#"
version = 1
domain = "x.com"
[[users]]
id = "u1"
name = "a"
protocol = "vless"
port = 443
reality = true
uuid = "abc"
"#;
        let spec: Spec = toml::from_str(toml).unwrap();
        assert_eq!(spec.users[0].protocol, Protocol::Vless);
        assert!(spec.users[0].reality);
    }

    #[test]
    fn require_reality_keys_enforces_pbk_single_source() {
        // 单一真相源:reality 用户缺 pbk → Err;有 pbk(即使 sid 空) → Ok
        let mut u = User::new("a", Protocol::Vless, 443, true, Transport::Tcp);
        assert!(u.require_reality_keys().is_err(), "缺 pbk 必须 Err");

        u.reality_pbk = "pbkXYZ".into();
        // sid 可空是合法的
        let (_pbk, sid) = u.require_reality_keys().unwrap();
        assert_eq!(sid, "");

        u.reality_sid = "sidABC".into();
        let (_pbk, sid) = u.require_reality_keys().unwrap();
        assert_eq!(sid, "sidABC");

        // is_renderable_reality 复用同一检查
        assert!(u.is_renderable_reality());
        u.reality_pbk.clear();
        assert!(!u.is_renderable_reality());
    }
}

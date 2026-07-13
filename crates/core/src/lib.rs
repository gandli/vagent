//! vagent 核心库:spec 解析、配置渲染、订阅、TLS 拼装。
//! 设计原则:core 不碰真实系统,只产出"意图"(命令、文件内容),
//! 由薄薄的 executor 去执行 —— 因此全部可在不联网、不 root 下断言。

pub mod core;
pub mod download;
pub mod error;
pub mod executor;
pub mod exit;
pub mod reality;
pub mod reality_scan;
pub mod render;
pub mod routing;
pub mod spec;
pub mod subscribe;
pub mod systemd;
pub mod tls;

pub use core::plan;
pub use error::Error;
pub use spec::{Cores, PortHopping, Protocol, Spec, Transport, User};

use std::path::Path;

/// 从 TOML 文件加载 Spec。
pub fn load_spec(path: &Path) -> Result<Spec, Error> {
    let s = std::fs::read_to_string(path)?;
    let spec: Spec = toml::from_str(&s)?;
    Ok(spec)
}

/// 持久化 Spec 到 TOML 文件(自动建父目录)。
pub fn save_spec(spec: &Spec, path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(spec)?;
    std::fs::write(path, s)?;
    Ok(())
}

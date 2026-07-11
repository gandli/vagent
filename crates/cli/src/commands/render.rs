//! 渲染内核配置到文件(供 xray/sing-box 直接加载或 CI 校验)。
//! `--core` 选内核,`--out` 指定输出路径(不填则打印到 stdout)。

use std::path::Path;
use vagent_core::core::{ProxyCore, SingboxCore, XrayCore};
use vagent_core::load_spec;

pub fn run(config: &Path, core: &str, out: Option<&str>) -> anyhow::Result<()> {
    let spec = load_spec(config)?;
    let rendered = match core.to_lowercase().as_str() {
        "xray" => XrayCore.render(&spec)?,
        "singbox" => SingboxCore.render(&spec)?,
        other => return Err(anyhow::anyhow!("未知内核: {other}")),
    };
    if let Some(path) = out {
        std::fs::write(path, &rendered.content)?;
        println!("已渲染 {core} 配置 → {path}");
    } else {
        println!("{}", rendered.content);
    }
    Ok(())
}

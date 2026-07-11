//! 订阅生成:多用户 v2rayN 订阅 bundle,可选服务端签名。

use std::path::Path;
use vagent_core::load_spec;
use vagent_core::spec::Spec;
use vagent_core::subscribe;

/// 生成订阅链接。sign=true 时用 config 父目录下的 secret 签名。
pub fn run(config: &Path, sign: bool) -> anyhow::Result<()> {
    let spec = match load_spec(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("加载配置失败 {}: {e}", config.display());
            std::process::exit(1);
        }
    };
    let bundle = subscribe::bundle(&spec).map_err(|e| anyhow::anyhow!(e))?;
    if !sign {
        println!("{bundle}");
        return Ok(());
    }
    // 签名:secret 跟随 config 父目录(root-optional)
    let base = Spec::base_dir(config);
    let secret_path = base.join("secret");
    let secret =
        subscribe::load_or_create_secret_at(&secret_path).map_err(|e| anyhow::anyhow!(e))?;
    let signed = subscribe::sign(&bundle, &secret);
    println!("{signed}");
    Ok(())
}

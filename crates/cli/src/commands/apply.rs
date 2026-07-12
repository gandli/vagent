use std::path::Path;
use vagent_core::load_spec;

/// 应用配置:渲染启用内核 → 写隔离路径 → 重载。
/// dry_run:只打印渲染结果,不落盘/不重载。
pub fn run(config: &Path, dry_run: bool) -> anyhow::Result<()> {
    let spec = match load_spec(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("加载配置失败 {}: {e}", config.display());
            return Err(anyhow::anyhow!("加载配置失败: {e}"));
        }
    };

    let rendered = vagent_core::plan(&spec, config)?;

    if dry_run {
        for r in &rendered {
            println!("=== {} ===", r.path);
            println!("{}", r.content);
        }
        return Ok(());
    }

    for r in &rendered {
        if let Some(parent) = Path::new(&r.path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&r.path, &r.content)?;
        println!("written: {}", r.path);
    }
    println!("reload: vagent-apply 已写盘(重载需 systemd 在 VPS 执行)");
    Ok(())
}

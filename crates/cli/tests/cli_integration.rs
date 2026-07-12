//! CLI 集成测试(黑盒)。
//! 设计原则:CLI 零命令行参数,`vagent` 直接进交互菜单。
//! 配置路径仅来自 VAGENT_CONFIG 环境变量或默认位置。
//! 菜单交互由 VAGENT_TEST_INPUT 环境变量驱动(每行一次输入:数字=菜单索引,文本=Input 答案)。
//! 非 tty 环境下若输入耗尽,菜单优雅退出。
//! 真实业务逻辑由 core crate 的单元测试覆盖。

use assert_cmd::Command;
use tempfile::tempdir;

/// 构造菜单输入序列(每行一次消费)。
/// 主菜单索引(对齐 v2ray-agent,0 基):
/// 0安装 1一键Reality 2Hysteria2 3REALITY 4Tuic 5用户 6证书 7分流 8订阅
/// 9内核 10应用 11状态 12卸载 13退出
/// 用户子菜单: 0新增 1列出 2删除 3链接 4返回
/// 订阅子菜单: 0生成 1签名 2返回
const FLOW_ADD_USER_AND_SUBSCRIBE: &str = "\
5
0
alice
443
0
4
8
0
2
13
";

#[test]
fn menu_flow_adds_user_and_generates_subscribe() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("vagent").join("spec.toml");

    let mut cmd = Command::cargo_bin("vagent").unwrap();
    cmd.env("HOME", tmp.path())
        .env("VAGENT_CONFIG", &cfg)
        .env("VAGENT_TEST_INPUT", FLOW_ADD_USER_AND_SUBSCRIBE);
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "vagent 菜单流应成功退出");

    // spec 应已生成并含 alice 用户
    assert!(cfg.exists(), "菜单首跑应生成默认配置: {}", cfg.display());
    let spec = std::fs::read_to_string(&cfg).unwrap();
    assert!(spec.contains("alice"), "spec 应含用户 alice:\n{spec}");

    // 订阅输出应包含 alice(从 stdout 捕获)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alice"), "订阅输出应含 alice:\n{stdout}");
}

#[test]
fn menu_no_input_exits_clean() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("nope").join("spec.toml");
    let assert = Command::cargo_bin("vagent")
        .unwrap()
        .env("HOME", tmp.path())
        .env("VAGENT_CONFIG", &cfg)
        .env("VAGENT_TEST_INPUT", "")
        .assert();
    assert.success();
    assert!(
        cfg.exists(),
        "vagent 首跑应引导生成默认配置: {}",
        cfg.display()
    );
}

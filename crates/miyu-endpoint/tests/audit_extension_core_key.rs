//! 审计复现（漏洞排查 2026-10-22）：扩展改配置（`config::set::set_from_package`）只看「键以 `<包编号>.` 开头、是清单里有的
//! 项、不是密钥」。包编号和核心模块同名的包（这里叫 `ui`），只要自己不声明 `[settings]`，`settle` 就不拦它
//! （`packages.rs` 的 `SettingsTaken` 只查声明了配置项的包），于是它改得了核心自己的系统配置 `ui.*`。换成别的核心模块名
//! 就是别的核心项（例如只有你能定的 `sandbox.tools.<名>`、`external.bindings.<外部身份>`），绕过了 X-30 / T-9 补的
//! 「扩展只能改自己的几项」。
//!
//! 期望（安全的样子）：核心模块同名的包改核心的项回 `local_only`，系统配置里什么都没写。修之前这条是红的。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 一个没声明配置项的扩展：编号由放在哪个文件夹定。
fn manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        r#"[package]
protocol = [1, 1]
name = {{ en = "Same name as a core module" }}

[command]
name = "ui-ext"
program = "{program}"
about = {{ en = "E" }}

[process]
args = [{}]
start = "manual"
"#,
        args.join(", ")
    )
}

#[tokio::test]
async fn a_package_named_like_a_core_module_cannot_set_core_settings() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "ui");
    let ask = format!(
        "ask:config.set:{}",
        json!({"layer": "system", "changes": [{"key": "ui.language", "value": "ja"}]})
    );
    let plan: Vec<&str> = vec![&keep, "hello", &ask, "listen"];
    home.write(
        "packages/ui/package.toml",
        &manifest(&program.name(), &steps(&plan)),
    );
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = call(&mut client, "extension.enable", "ui").await;
    assert!(enabled.get("result").is_some(), "开得了：{enabled}");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    let answer = loop {
        let answers: Vec<Value> = read(&path)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| line.get("result").is_some() || line.get("error").is_some())
            .collect();
        // 握手一条，config.set 一条。
        if answers.len() >= 2 {
            break answers[1].clone();
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    let config = read(&home.root.path().join("config.toml"));
    core.stop_extensions().await;
    assert_eq!(
        answer["error"]["data"]["reason"], "local_only",
        "叫 ui 的扩展改了核心的 ui.language：{answer}\n系统配置：{config}"
    );
    assert!(!config.contains("ja"), "系统配置里写进去了：{config}");
}

//! 环境那几块能关（施工 C-11，`docs/blueprint/presets.md`「环境那几块」）：真核心走一遍。预设关了的功能，回合开始不注入那一块
//! 事实，system 里跟着它的那一行不拼；只关工作目录的环境那一块照拆开的写法只写时间；全部功能的照旧都有。

use serde_json::json;

use miyu_kernel::request::Request;

use crate::preset_face::first_request;
use crate::support::*;

/// 请求里的全部字（system 加上消息），照调试的写法：事实在消息里。
fn all_of(request: &Request) -> String {
    format!("{request:?}")
}

fn line(name: &str) -> String {
    let text = std::fs::read_to_string(default_resources().join("core").join(name))
        .expect("读得出核心的那一行");
    text.trim_end().split('{').next().expect("有字").to_string()
}

#[tokio::test]
async fn everything_on_keeps_every_block_and_line() {
    let home = Home::new();
    let (_, request) = first_request(&home, json!({"cwd": "~", "persona": "engineer"})).await;
    let all = all_of(&request);
    // 测试的核心没配模型，没有模型那一块（关掉它的那一条在内核的测试里）。
    for block in ["<env time=", "cwd=", "<permission level=", "<session id="] {
        assert!(all.contains(block), "少了 {block}：{all}");
    }
    for name in [
        "permission-rule.txt",
        "local-paths-rule.txt",
        "machine.txt",
        "agent-version.txt",
    ] {
        assert!(
            request.system.contains(&line(name)),
            "少了 {name}：{}",
            request.system
        );
    }
}

#[tokio::test]
async fn a_preset_turns_the_blocks_and_their_lines_off() {
    let home = Home::new();
    home.write(
        "home/alice/presets/group.toml",
        "[preset]\nname = { en = \"Group\" }\n\n[features]\nenv-workdir = false\nenv-permission = false\nenv-session = false\nenv-model = false\nenv-harness = false\nenv-machine = false\n",
    );
    let (_, request) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "group"}),
    )
    .await;
    let all = all_of(&request);
    assert!(all.contains("<env time="), "时间开着：{all}");
    for gone in [
        "cwd=",
        "<permission level=",
        "<session id=",
        "<model provider=",
    ] {
        assert!(!all.contains(gone), "关了的 {gone} 还在：{all}");
    }
    for name in [
        "permission-rule.txt",
        "local-paths-rule.txt",
        "machine.txt",
        "agent-version.txt",
    ] {
        assert!(
            !request.system.contains(&line(name)),
            "关了的 {name} 还在：{}",
            request.system
        );
    }
    // 时间也关了：环境那一块整块没有。
    home.write(
        "home/alice/presets/silent.toml",
        "[preset]\nname = { en = \"Silent\" }\n\n[features]\nenv-time = false\nenv-workdir = false\n",
    );
    let (_, silent) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "silent"}),
    )
    .await;
    let all = all_of(&silent);
    assert!(!all.contains("<env "), "两样都关了不注入：{all}");
    assert!(all.contains("<permission level="), "别的照旧：{all}");
}

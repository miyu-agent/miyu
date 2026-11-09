//! 本会话放行过的（施工 D-1，`docs/blueprint/session/guard.md`「判一次调用」第 7 条）：人选了「本会话都允许」，以后读那个目录
//! 和它下面的不再问；名字只是开头一样的旁边的目录照旧问；家目录里的一个文件只放它本身。A16 起文件工具问人的只剩只读时读别的
//! 程序的私密目录（施工 LV-1），这里照它测：只读开局，私密目录照这一轮的配置加。
//!
//! 场地照 `guard.rs`：数据根 `data/`、假的家 `home/`、工作区 `work/`。

use std::sync::Arc;

use miyu_kernel::event::{Body, Decision, Event, Level, Permission};
use miyu_kernel::id::CallId;
use miyu_kernel::session::{Answer, Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 场地里的一处，写成她会给的样子（绝对路径）。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

fn file(home: &Home, path: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, "x").expect("写得进");
}

/// 调一次读文件的假工具 `read`，参数里是一条路径。
fn read(path: &str) -> Play {
    Play::calls(&[("read", &serde_json::json!({ "path": path }).to_string())])
}

/// 私密目录加上家目录下的 `private`（写成 `~/…`）：读它们要问人。
fn private(home: &mut Home, private: &[&str]) {
    let listed: Vec<String> = private.iter().map(|dir| format!("\"~/{dir}\"")).collect();
    home.configs = crate::support::routing::configs_with(
        "",
        &format!("[sandbox.private]\nadd = [{}]\n", listed.join(", ")),
        &[],
    );
}

/// 在工作区 `work/` 里造一个只读、有人能确认的会话，只有一件 `read`。
async fn session(home: &Home, script: &Script, read: &Arc<Fake>) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: true,
        },
        attended: true,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: None,
    };
    let catalog = Catalog::new([Arc::clone(read) as Arc<dyn Tool>]).expect("合写法");
    home.create_as(script, &catalog, opening).await
}

/// 日志里的确认请求：调用编号和提的规则，照先后。
fn requests(log: &[Event]) -> Vec<(CallId, serde_json::Value)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some((
                request.call_id,
                serde_json::from_str(request.rule.as_ref().expect("提了规则").get())
                    .expect("是 JSON"),
            )),
            _ => None,
        })
        .collect()
}

/// 等到日志里有 `n` 条确认请求，交回最后那一条。
async fn nth_request(home: &Home, handle: &Handle, n: usize) -> (CallId, serde_json::Value) {
    let session = handle.id().clone();
    until_logged(home, &session, |log| requests(log).len() >= n).await;
    requests(&home.log(&session)).pop().expect("有确认请求")
}

/// 人回答第 `call_id` 的确认：本会话都允许。
async fn allow_for_the_session(handle: &Handle, id: &str, call_id: CallId) {
    let answer = Answer::Approval {
        decision: Decision::Session,
        reason: None,
        attachments: Vec::new(),
        nested: None,
    };
    ask(handle, id, Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
}

async fn interrupt(handle: &Handle) {
    ask(
        handle,
        "cmd-9",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
}

fn real(home: &Home, path: &str) -> String {
    std::fs::canonicalize(home.scratch.0.join(path))
        .expect("在")
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
async fn allowing_for_the_session_lets_later_reads_under_that_directory_through() {
    let mut home = Home::outside_temp();
    for path in [
        "home/vault/a.txt",
        "home/vault/sub/b.txt",
        "home/vaultx/c.txt",
    ] {
        file(&home, path);
    }
    private(&mut home, &["vault", "vaultx"]);
    let fake = Fake::new("read", Access::Read, Act::Echo);
    let script = Script::new([
        read(&at(&home, "home/vault/a.txt")),
        read(&at(&home, "home/vault/sub/b.txt")),
        read(&at(&home, "home/vaultx/c.txt")),
    ]);
    let handle = session(&home, &script, &fake).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let (call_id, rule) = nth_request(&home, &handle, 1).await;
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [real(&home, "home/vault")]})
    );
    allow_for_the_session(&handle, "cmd-2", call_id).await;
    // 下面的子目录不问，直接跑；名字只是开头一样的 `vaultx/` 照旧问。
    let (_, rule) = nth_request(&home, &handle, 2).await;
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [real(&home, "home/vaultx")]})
    );
    assert_eq!(fake.calls().len(), 2, "a.txt、sub/b.txt 都跑了");
    interrupt(&handle).await;
    until_turn_ends(&mut pushes).await;
}

/// 私密的是家目录里的一个文件（出厂的 `~/.netrc` 这种）。
#[tokio::test]
async fn a_file_right_in_the_home_is_allowed_by_itself() {
    let mut home = Home::outside_temp();
    file(&home, "home/a.txt");
    file(&home, "home/b.txt");
    private(&mut home, &["a.txt", "b.txt"]);
    let fake = Fake::new("read", Access::Read, Act::Echo);
    let script = Script::new([
        read(&at(&home, "home/a.txt")),
        read(&at(&home, "home/a.txt")),
        read(&at(&home, "home/b.txt")),
    ]);
    let handle = session(&home, &script, &fake).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 所在的目录是家目录本身：规则里只有这个文件，家目录里有钥匙。
    let (call_id, rule) = nth_request(&home, &handle, 1).await;
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [real(&home, "home/a.txt")]})
    );
    allow_for_the_session(&handle, "cmd-2", call_id).await;
    // 同一个文件再读不问；家目录里别的文件照旧问。
    let (_, rule) = nth_request(&home, &handle, 2).await;
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [real(&home, "home/b.txt")]})
    );
    assert_eq!(fake.calls().len(), 2);
    interrupt(&handle).await;
    until_turn_ends(&mut pushes).await;
}

/// 家目录在链接后面（施工 D-1，CI run 960 在 Windows 上撞见同一类：真实的位置带 `\\?\` 前缀、家目录没换）：放行的范围拿
/// 真实的位置比，家目录里的文件照样只放它本身。只在 Unix 上造得出链接。
#[cfg(unix)]
#[tokio::test]
async fn a_home_behind_a_link_is_compared_by_its_real_place() {
    let mut home = Home::outside_temp();
    let link = home.scratch.0.join("home");
    std::fs::remove_dir_all(&link).expect("假的家是空目录，删得掉");
    std::fs::create_dir_all(home.scratch.0.join("real-home")).expect("建得了");
    std::os::unix::fs::symlink(home.scratch.0.join("real-home"), &link).expect("建得了链接");
    file(&home, "home/a.txt");
    private(&mut home, &["a.txt"]);
    let fake = Fake::new("read", Access::Read, Act::Echo);
    let script = Script::new([read(&at(&home, "home/a.txt"))]);
    let handle = session(&home, &script, &fake).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let (_, rule) = nth_request(&home, &handle, 1).await;
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [real(&home, "home/a.txt")]})
    );
    interrupt(&handle).await;
    until_turn_ends(&mut pushes).await;
}

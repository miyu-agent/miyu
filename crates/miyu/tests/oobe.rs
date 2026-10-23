//! OOBE 注册机主（施工 Y-12 中上，设计 06 U22）：还没有机主的真核心只跑设置那一段：本机令牌握手说要设置、不是谁，别的方法
//! `setup_first`，`admin` 注册不成；注册成了交回登录令牌，这个连接断开；头重连进来就是他，开的会话在他的家目录里。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::support::{Home, count, within};
use miyu_ipc::connect_or_start;

/// 一条连接：一行一条地发、收。
struct Line {
    read: BufReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Line {
    fn new(connection: miyu_ipc::Connection) -> Line {
        let (read, write) = tokio::io::split(connection);
        Line {
            read: BufReader::new(read),
            write,
        }
    }

    /// 发一条请求，等它的回应（推送跳过）。
    async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let got = self.next().await.expect("没断开");
            if got["id"] == id {
                return got;
            }
        }
    }

    /// 下一行；断开了的没有。
    async fn next(&mut self) -> Option<Value> {
        let mut line = String::new();
        let read = within("下一行", self.read.read_line(&mut line))
            .await
            .expect("读得了");
        (read > 0).then(|| serde_json::from_str(&line).expect("是 JSON"))
    }
}

fn hello(token: &str) -> Value {
    json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
           "caps": {"input": false}, "token": token})
}

#[tokio::test]
async fn the_owner_registers_on_first_run_and_the_core_is_theirs_after() {
    let home = Home::bare();
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let mut first = Line::new(connection);
    let reply = first.call("h1", "hello", hello(&token)).await;
    assert_eq!(reply["result"]["setup"], json!(true), "{reply}");
    assert_eq!(reply["result"]["account"], Value::Null, "还不是谁：{reply}");
    let refused = first.call("l1", "session.list", json!({})).await;
    assert_eq!(
        refused["error"]["data"]["reason"], "setup_first",
        "{refused}"
    );
    let password = "correct horse battery";
    let taken = first
        .call(
            "s1",
            "account.setup",
            json!({"username": "admin", "password": password}),
        )
        .await;
    assert_eq!(
        taken["error"]["data"]["reason"], "username_taken",
        "admin 是系统：{taken}"
    );
    let set = first
        .call(
            "s2",
            "account.setup",
            json!({"username": "shorin", "password": password}),
        )
        .await;
    assert_eq!(set["result"]["account"], "shorin", "{set}");
    assert!(set["result"]["login"]["token"].is_string(), "{set}");
    assert!(first.next().await.is_none(), "答完就断");

    let (connection, token) = within("重连", connect_or_start(&home.root, || home.core()))
        .await
        .expect("连得上");
    let mut again = Line::new(connection);
    let reply = again.call("h2", "hello", hello(&token)).await;
    assert_eq!(
        reply["result"]["account"], "shorin",
        "本机令牌就是机主：{reply}"
    );
    assert!(reply["result"].get("setup").is_none(), "{reply}");
    let created = again
        .call(
            "c1",
            "session.create",
            json!({"cwd": home.dir.to_string_lossy()}),
        )
        .await;
    let session = created["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{created}"))
        .to_string();
    let shorin = miyu_kernel::id::AccountId::parse("shorin").expect("合写法");
    let id = miyu_kernel::id::SessionId::parse(&session).expect("合写法");
    assert!(
        home.root.session_dir(&shorin, &id).is_dir(),
        "会话在机主的家目录里"
    );
    let admin = home.root.account_dir(&miyu_core::admin());
    let inside: Vec<_> = std::fs::read_dir(&admin)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    assert!(!admin.exists(), "admin 的家目录没建：{inside:?}");
    drop(again);
    home.until_stopped().await;
}

/// 设置那一段也照空闲时限退（施工 Y-12 中上修）：头拉起它、没注册就走了，核心不一直挂着。
#[tokio::test]
async fn an_unattended_setup_core_leaves_when_idle() {
    let home = Home::bare();
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let mut line = Line::new(connection);
    let reply = line.call("h1", "hello", hello(&token)).await;
    assert_eq!(reply["result"]["setup"], json!(true), "{reply}");
    drop(line);
    home.until_stopped().await;
    let log = home.core_log();
    assert_eq!(count(&log, "stopped reason=idle"), 1, "空闲了自己走：{log}");
    assert!(!home.root.account_dir(&miyu_core::admin()).exists());
}

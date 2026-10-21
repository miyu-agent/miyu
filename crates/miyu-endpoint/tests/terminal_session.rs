//! 终端会话（施工 8-33，设计 `22-命令行.md` 第七节）：`session.terminal` 没有的开一个、之后一直是它；`session.set_terminal`
//! 指到选中的会话，只收自己的本机主会话；闲置超过 `terminal.idle_new_after` 的开新的、回应带上次的时刻，旧的不动；带 `fresh` 的
//! 不管指着谁都开新的（施工 8-33 再补）；切过去的那一刻算一次动静，切回闲置了很久的不再被换掉（施工 T-1）。

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 要终端会话，交回回应的结果。
async fn terminal(client: &mut Client, id: &str, params: Value) -> Value {
    let reply = client.call(id, "session.terminal", params).await;
    reply["result"].clone()
}

#[tokio::test]
async fn the_terminal_session_is_made_once_and_can_be_pointed_elsewhere() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let first = terminal(&mut client, "t1", json!({"cwd": cwd})).await;
    assert_eq!(first["created"], json!(true), "{first}");
    let session = first["session"].as_str().expect("有编号").to_string();
    let again = terminal(&mut client, "t2", json!({})).await;
    assert_eq!(again, json!({"session": session}), "之后一直是它");

    let other = client.create("c1", &cwd).await;
    let pointed = client
        .call("p1", "session.set_terminal", json!({"session": other}))
        .await;
    assert_eq!(pointed["result"], json!({"session": other}), "{pointed}");
    assert_eq!(
        terminal(&mut client, "t3", json!({})).await,
        json!({"session": other})
    );
    let missing = client
        .call(
            "p2",
            "session.set_terminal",
            json!({"session": "01a0f233-cfec-7023-8ed5-2a037a1d5ec8"}),
        )
        .await;
    assert_eq!(reason(&missing), Some("session_not_found"), "{missing}");

    // 指着的删了：再要的时候开一个新的。
    let deleted = client
        .call("d1", "session.delete", json!({"session": other}))
        .await;
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    let fresh = terminal(&mut client, "t4", json!({})).await;
    assert_eq!(fresh["created"], json!(true), "{fresh}");
    assert_ne!(fresh["session"], json!(other));
}

#[tokio::test]
async fn fresh_opens_a_new_one_whatever_is_pointed_at() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let first = terminal(&mut client, "t1", json!({"cwd": cwd})).await;
    let old = first["session"].as_str().expect("有编号").to_string();
    let fresh = terminal(&mut client, "t2", json!({"cwd": cwd, "fresh": true})).await;
    assert_eq!(fresh["created"], json!(true), "{fresh}");
    assert_ne!(fresh["session"], json!(old), "指着的还新鲜也开新的");
    assert_eq!(
        terminal(&mut client, "t3", json!({})).await,
        json!({"session": fresh["session"]}),
        "指针挪过去了"
    );
    let kept = client.call("l1", "session.list", json!({})).await;
    assert!(kept.to_string().contains(&old), "旧的不动：{kept}");
}

#[tokio::test]
async fn an_idle_terminal_session_is_replaced_and_kept() {
    let home = Home::new();
    let session = {
        let mut client = Client::connect(home.core(&Script::new([])));
        client.hello().await;
        let made = terminal(&mut client, "t1", json!({})).await;
        made["session"].as_str().expect("有编号").to_string()
    };
    // 这个会话和指针拷进一个新的数据根，事件都挪到很早以前：那里没有会话列表的索引（派生的，照日志建），起来时它闲置了很久。
    // 不在原处删索引：头一个核心还开着它，Windows 上删不掉开着的文件（CI run 39320472077）。
    let id = miyu_kernel::id::SessionId::parse(&session).expect("编号");
    let later = Home::new();
    let from = home.root.session_dir(&alice(), &id);
    let to = later.root.session_dir(&alice(), &id);
    std::fs::create_dir_all(&to).expect("建得了目录");
    for entry in std::fs::read_dir(&from).expect("有目录").flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") {
            let text = std::fs::read_to_string(&path).expect("读得了");
            std::fs::write(to.join(entry.file_name()), regex_lite_at(&text)).expect("写得进");
        }
    }
    let pointer = home.root.account_dir(&alice()).join("terminal.json");
    // 指针上指过去的时刻也挪到那时（施工 T-1）。
    let written = std::fs::read_to_string(&pointer).expect("记下了指针");
    std::fs::write(
        later.root.account_dir(&alice()).join("terminal.json"),
        regex_lite_at(&written),
    )
    .expect("写得进");

    let mut client = Client::connect(later.core(&Script::new([])));
    client.hello().await;
    client.call("l1", "session.list", json!({})).await;
    let renewed = terminal(&mut client, "t2", json!({})).await;
    assert_ne!(renewed["session"], json!(session), "开了新的：{renewed}");
    assert_eq!(
        renewed["renewed"],
        json!({"last": "2026-01-01T00:00:00.000Z"}),
        "{renewed}"
    );
    assert!(to.is_dir(), "旧的不动");

    // 主动切回那个闲置了很久的（施工 T-1）：切过去那一刻算一次动静，下一次要的还是它，不换。
    let pointed = client
        .call("p1", "session.set_terminal", json!({"session": session}))
        .await;
    assert_eq!(pointed["result"], json!({"session": session}), "{pointed}");
    assert_eq!(
        terminal(&mut client, "t3", json!({})).await,
        json!({"session": session}),
        "切过去的不再被闲置换掉"
    );
}

/// 每一行的 `"at":"…"` 都换成 2026-01-01 零点。
fn regex_lite_at(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("\"at\":\"") {
        let value = start + "\"at\":\"".len();
        out.push_str(&rest[..value]);
        out.push_str("2026-01-01T00:00:00.000Z");
        let end = rest[value..]
            .find('"')
            .map_or(rest.len(), |end| value + end);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

//! 导出会话的原始包（施工 SE-1 上，`docs/blueprint/protocol.md`「`session.export`」）：真核心走一遍。包里的段和数据根里的
//! 一个字节不差；导主会话连着挂在它下面的支线整份带；导分叉出来的连着来源到分叉点为止的那几段；写到数据根里、已经有的、目录不在、相对路径、没有这个会话的拒。

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};
use zip::ZipArchive;

use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use miyu_store::bundle::Manifest;

use crate::support::*;

async fn export(client: &mut Client, id: &str, session: &str, path: &Path) -> Value {
    client
        .call(
            id,
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": path}}),
        )
        .await
}

fn entry(path: &Path, name: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(File::open(path).expect("包在")).expect("是 ZIP");
    let mut file = archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("包里有 {name}"));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("读得出");
    bytes
}

fn manifest(path: &Path) -> Manifest {
    serde_json::from_slice(&entry(path, "manifest.json")).expect("清单读得回")
}

/// 会话目录里的第一段：名字和字节（分叉出来的会话第一段照分叉点的下一条起名）。
fn first_segment(home: &Home, session: &str) -> (String, Vec<u8>) {
    let dir = home
        .root
        .session_dir(&SessionId::parse(session).expect("合写法"));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("读得了")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".jsonl"))
        .collect();
    names.sort();
    let name = names.first().expect("至少一段").clone();
    let bytes = std::fs::read(dir.join(&name)).expect("读得出");
    (name, bytes)
}

/// 包里这个会话的那一段，和数据根里的对一遍。
fn same_segment(out: &Path, home: &Home, session: &str, why: &str) {
    let (name, bytes) = first_segment(home, session);
    assert_eq!(
        entry(out, &format!("sessions/{session}/{name}")),
        bytes,
        "{why}"
    );
}

#[tokio::test]
async fn a_session_goes_out_byte_for_byte_and_a_fork_brings_its_source_up_to_the_fork() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "主线一").await;
    home.until_turns(&session, 1).await;
    let forked = client
        .call("f1", "session.fork", json!({"session": session}))
        .await;
    let branch = forked["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{forked}"))
        .to_string();
    client.say("s2", &branch, "支线").await;
    home.until_turns(&branch, 1).await;
    client.say("s3", &session, "主线二").await;
    home.until_turns(&session, 2).await;

    let out = home.work.join("main.zip");
    let reply = export(&mut client, "e1", &session, &out).await;
    assert_eq!(
        reply["result"]["sessions"],
        json!([session, branch]),
        "分叉出来的支线挂在它下面，一起带走：{reply}"
    );
    assert_eq!(
        reply["result"]["bytes"],
        json!(std::fs::metadata(&out).expect("写出来了").len())
    );
    same_segment(&out, &home, &session, "一个字节不改");
    let main = manifest(&out);
    assert_eq!(main.main.as_str(), session);
    same_segment(&out, &home, &branch, "支线是后代，整份带");

    let out = home.work.join("branch.zip");
    let reply = export(&mut client, "e2", &branch, &out).await;
    assert_eq!(
        reply["result"]["sessions"],
        json!([branch, session]),
        "{reply}"
    );
    let both = manifest(&out);
    let source = &both.sessions[1];
    let upto = source.upto.expect("来源写了取到哪");
    let mainline = String::from_utf8(entry(
        &out,
        &format!("sessions/{session}/000000000001.jsonl"),
    ))
    .expect("UTF-8");
    assert!(
        mainline.contains("主线一") && !mainline.contains("主线二"),
        "来源只到分叉点"
    );
    let last: Value = serde_json::from_str(mainline.lines().last().expect("有行")).expect("JSON");
    assert_eq!(last["seq"], json!(upto));
}

#[tokio::test]
async fn bad_places_and_unknown_sessions_are_refused() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;

    let taken = home.work.join("taken.zip");
    std::fs::write(&taken, "人家的").expect("写得进");
    let cases = [
        (taken.clone(), "export_exists"),
        (home.root.path().join("inside.zip"), "export_in_data_root"),
        (
            home.work.join("no-such-dir").join("x.zip"),
            "export_unwritable",
        ),
    ];
    for (n, (path, reason)) in cases.iter().enumerate() {
        let reply = export(&mut client, &format!("e{n}"), &session, path).await;
        assert_eq!(reply["error"]["data"]["reason"], json!(reason), "{reply}");
    }
    assert_eq!(std::fs::read_to_string(&taken).expect("在"), "人家的");
    let relative = client
        .call(
            "r1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": "x.zip"}}),
        )
        .await;
    assert_eq!(
        relative["error"]["data"]["reason"],
        json!("bad_params"),
        "{relative}"
    );
    let inline = client
        .call(
            "m1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": "inline"}),
        )
        .await;
    assert_eq!(
        inline["error"]["data"]["reason"],
        json!("bad_params"),
        "原始包只写文件"
    );
    let missing = export(
        &mut client,
        "n1",
        "0192f3a0-0000-7000-8000-000000000099",
        &home.work.join("none.zip"),
    )
    .await;
    assert_eq!(
        missing["error"]["data"]["reason"],
        json!("session_not_found"),
        "{missing}"
    );
}

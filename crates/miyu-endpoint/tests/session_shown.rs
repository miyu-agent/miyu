//! 展示的文件不拷进核心（施工 SW-1，`docs/construction/SW-1-展示不拷不限大小.md`）：`show` 一个过了 20 MiB 的文件照常成，
//! `file.shown` 只记路径、大小、修改时间，不存 blob；`session.shown` 照会话加调用编号分段读它，本机、远程的连接都读得到；
//! 读到结尾、只问大小、一段太长；没展示过的、没有的会话一律 `unknown_shown`，写了路径的参数不对；文件改了 `file_changed`，
//! 没了 `file_gone`。

use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 展示的文件多大：过了以前的 20 MiB 上限。
const BIG: u64 = 20 * 1024 * 1024 + 1;

/// 文件开头：MP4 的 `ftyp`，照它认成视频。
const HEAD: &[u8] = b"\0\0\0\x18ftypisom";

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 工作目录里一份稀疏的大文件：开头是 MP4 的文件头 [`HEAD`]、末尾 `tail`，中间是空的，不真占盘。
fn big_file(home: &Home) {
    let mut file = std::fs::File::create(home.work.join("big.mp4")).expect("建得了");
    file.write_all(HEAD).expect("写得进");
    file.set_len(BIG).expect("撑得大");
    file.seek(SeekFrom::Start(BIG - 4)).expect("挪得到末尾");
    file.write_all(b"tail").expect("写得进");
}

/// 她读一次 `a.txt`、展示一次 `big.mp4`；交回核心、本机的连接、会话。
async fn shown(home: &Home) -> (Arc<Core>, Client, String) {
    std::fs::write(home.work.join("a.txt"), "a\n").expect("写得进");
    big_file(home);
    let plays = vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("show", r#"{"path":"big.mp4"}"#)]),
        Play::Says("好了。"),
    ];
    let core = home.core_with_tools(&Script::new(plays), tools(), TOKEN);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "给我看").await;
    home.until_turns(&session, 1).await;
    (core, client, session)
}

/// 网页软件替远程的人开的连接（同 `local_machine.rs`）。
async fn remote(core: Arc<Core>) -> Client {
    let mut client = Client::connect(core);
    let notice =
        json!({"jsonrpc": "2.0", "method": "connection.remote", "params": {"from": "203.0.113.9"}});
    client.line(&notice.to_string()).await;
    client.hello().await;
    client
}

/// 日志里的工具结果：调用编号和效果原样的 JSON，照先后。
fn results(home: &Home, session: &str) -> Vec<(String, Value)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some((
                result.call_id.to_string(),
                serde_json::to_value(&result.effects).expect("写得成"),
            )),
            _ => None,
        })
        .collect()
}

/// 目录 `dir` 底下（一层层往下）最大的文件多大；没有这个目录的是 0。
fn largest(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => largest(&entry.path()),
            _ => entry.metadata().map_or(0, |meta| meta.len()),
        })
        .max()
        .unwrap_or(0)
}

async fn read(client: &mut Client, params: Value) -> Value {
    client.call("s", "session.shown", params).await
}

/// 读成的字节和整份多大；拒了的是原因。
fn got(reply: &Value) -> Result<(Vec<u8>, u64), String> {
    match reply["result"]["data"].as_str() {
        Some(data) => Ok((
            STANDARD.decode(data).expect("是 base64"),
            reply["result"]["size"].as_u64().expect("有大小"),
        )),
        None => Err(reason(reply).unwrap_or("没有原因").to_string()),
    }
}

#[tokio::test]
async fn a_big_file_is_shown_by_path_and_read_in_pieces_from_far_away() {
    let home = Home::new();
    let (core, mut local, session) = shown(&home).await;
    let results = results(&home, &session);
    let (call, effects) = &results[1];
    let shown = &effects[0];
    assert_eq!(shown["kind"], "file.shown", "{effects}");
    assert_eq!(
        shown["path"],
        json!(home.work.join("big.mp4").to_string_lossy()),
        "{shown}"
    );
    assert_eq!(shown["size"], json!(BIG), "{shown}");
    assert_eq!(shown["media_type"], "video/mp4", "{shown}");
    assert!(shown["modified"].is_string(), "记修改时间：{shown}");
    assert!(shown.get("blob").is_none(), "不存 blob：{shown}");
    assert!(
        largest(&home.root.blobs()) < BIG,
        "存处里没有存这一份（别的小东西不管）"
    );

    let mut far = remote(core).await;
    let at = |offset: u64, length: u64| json!({"session": session, "call": call, "offset": offset, "length": length});
    assert_eq!(
        got(&read(&mut far, at(0, 12)).await),
        Ok((HEAD.to_vec(), BIG))
    );
    assert_eq!(
        got(&read(&mut far, at(BIG - 4, 512 * 1024)).await),
        Ok((b"tail".to_vec(), BIG)),
        "读到结尾就停"
    );
    assert_eq!(
        got(&read(&mut far, at(BIG, 4)).await),
        Ok((Vec::new(), BIG)),
        "过了结尾是空的"
    );
    assert_eq!(
        got(&read(&mut far, at(0, 0)).await),
        Ok((Vec::new(), BIG)),
        "写 0 只问大小"
    );
    let whole = read(&mut far, json!({"session": session, "call": call})).await;
    assert_eq!(
        got(&whole).map(|(data, _)| data.len()),
        Ok(512 * 1024),
        "不写的从头读 512 KiB"
    );
    assert_eq!(
        reason(&read(&mut far, at(0, 512 * 1024 + 1)).await),
        Some("bad_params"),
        "一段最多 512 KiB"
    );
    assert_eq!(
        got(&read(&mut local, at(0, 12)).await),
        Ok((HEAD.to_vec(), BIG)),
        "本机的连接也读得到"
    );
}

#[tokio::test]
async fn only_what_she_showed_can_be_read() {
    let home = Home::new();
    let (_core, mut client, session) = shown(&home).await;
    let results = results(&home, &session);
    let (read_call, shown_call) = (&results[0].0, &results[1].0);
    for (params, want) in [
        (
            json!({"session": session, "call": read_call}),
            "unknown_shown",
        ),
        (
            json!({"session": session, "call": "call_999_1"}),
            "unknown_shown",
        ),
        (
            json!({"session": "01900000-0000-7000-8000-00000000abcd", "call": shown_call}),
            "unknown_shown",
        ),
        (json!({"session": session, "call": "nope"}), "bad_params"),
        (json!({"session": "nope", "call": shown_call}), "bad_params"),
        (
            json!({"session": session, "call": shown_call, "path": "/etc/hosts"}),
            "bad_params",
        ),
    ] {
        let reply = read(&mut client, params.clone()).await;
        assert_eq!(reason(&reply), Some(want), "{params}：{reply}");
    }
}

#[tokio::test]
async fn a_changed_or_missing_file_is_refused() {
    let home = Home::new();
    let (_core, mut client, session) = shown(&home).await;
    let call = results(&home, &session)[1].0.clone();
    let params = json!({"session": session, "call": call, "length": 4});
    let path = home.work.join("big.mp4");
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("打得开");
    let modified = file
        .metadata()
        .and_then(|meta| meta.modified())
        .expect("有修改时间");
    file.set_modified(modified + std::time::Duration::from_secs(5))
        .expect("改得了时间");
    assert_eq!(
        reason(&read(&mut client, params.clone()).await),
        Some("file_changed"),
        "大小没变、时间变了"
    );
    file.set_modified(modified).expect("改得回去");
    assert!(
        got(&read(&mut client, params.clone()).await).is_ok(),
        "改回去又对得上"
    );
    file.set_len(BIG + 1).expect("撑得大");
    file.set_modified(modified).expect("改得回去");
    drop(file);
    assert_eq!(
        reason(&read(&mut client, params.clone()).await),
        Some("file_changed"),
        "时间没变、大小变了"
    );
    std::fs::remove_file(&path).expect("删得掉");
    assert_eq!(
        reason(&read(&mut client, params.clone()).await),
        Some("file_gone")
    );
    // 换成一个链接（指向别的文件）的也不跟过去：照没了拒。
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(home.work.join("a.txt"), &path).expect("建得了链接");
        assert_eq!(reason(&read(&mut client, params).await), Some("file_gone"));
    }
}

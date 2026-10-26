//! 附件照连接的账号存取（施工 Y-2中下，`protocol.md` 的 `blob.put`、`blob.open`、`blob.get`、`model.call`）：以前一律存进、读自
//! 管理员名下，声明了系统账号的桥传的也在管理员那里。现在桥传的（一次传、分块传）都在它自己名下，管理员名下没有；不带
//! `session` 的 `blob.get`、`model.call` 的图照它自己名下找；它往主人的私聊里发的附件拷一份进主人名下，她照属主读。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use miyu_kernel::id::{AccountId, ContentHash};
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;

use crate::attach::png;
use crate::blob_session::{got, on_qq, replies, served_core};
use crate::support::extensions::*;
use crate::support::*;

/// 这个账号名下有没有这个 blob。
fn holds(home: &Home, account: &AccountId, blob: &str) -> bool {
    let hash = ContentHash::parse(blob).expect("合写法");
    Blobs::new(home.root.blobs(account)).path(&hash).is_file()
}

/// 回应没有出错。
fn fine(reply: &Value) -> bool {
    reply.get("error").is_none() && reply.get("result").is_some()
}

#[tokio::test]
async fn the_bridges_uploads_stay_under_the_bridge_and_reach_the_owner_when_sent() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let picture = png(2, 2);
    let chunked = b"sent in one chunk";
    let [p, u] =
        [&picture[..], &chunked[..]].map(|bytes| ContentHash::of(bytes).as_str().to_string());
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!(
                r#"ask:blob.put:{{"data":"{}","name":"a.png"}}"#,
                STANDARD.encode(&picture)
            ),
            &format!(
                r#"ask:blob.open:{{"name":"b.txt","size":{}}}"#,
                chunked.len()
            ),
            &format!(
                r#"ask:blob.write:{{"upload":"{{upload}}","offset":0,"data":"{}"}}"#,
                STANDARD.encode(chunked)
            ),
            r#"ask:blob.close:{"upload":"{upload}"}"#,
            &format!(r#"ask:blob.get:{{"blob":"{u}"}}"#),
            &format!(
                r#"ask:model.call:{{"purpose":"look","messages":[{{"role":"user","text":"看","images":["{p}"]}}]}}"#
            ),
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
            &format!(
                r#"ask:session.send:{{"session":"{{session}}","text":"看这个","as":{{"external":"qq:10001"}},"attachments":[{{"blob":"{p}","name":"a.png","media_type":"image/png"}}]}}"#
            ),
            "wait",
        ]),
        "always",
        true,
    );
    on_qq(&home);
    let script = Script::new([Play::Says("嗯。")]);
    let _core = served_core(&home, &script);
    let read = replies(&path, 9).await;
    let bot = AccountId::parse("bot").expect("合写法");

    assert_eq!(read[1]["result"]["blob"], p.as_str(), "{read:?}");
    assert_eq!(read[4]["result"]["blob"], u.as_str(), "{read:?}");
    assert!(
        holds(&home, &bot, &p) && holds(&home, &bot, &u),
        "存在桥名下"
    );
    assert!(!holds(&home, &alice(), &u), "管理员名下没有");
    assert_eq!(
        got(&read[5]),
        Ok(chunked.to_vec()),
        "不带会话的照自己名下读"
    );
    // 这个核心没配模型：图先照自己名下找到了，才轮到找模型。
    assert_eq!(
        read[6]["error"]["data"]["reason"], "no_model",
        "看图照自己名下找：{:?}",
        read[6]
    );
    assert_eq!(read[7]["result"]["account"], "alice", "主人的私聊归管理员");
    assert!(fine(&read[8]), "发进主人的私聊：{:?}", read[8]);
    assert!(holds(&home, &alice(), &p), "发出去的拷进了属主名下");
}

/// 替机主干活的扩展（不是平台桥，施工 BUG-2，onebot 偷表情包时撞上）：`blob.put` 存在扩展自己名下，`model.call` 带这张图
/// 先照连接自己的账号找、找得到，拷一份进替谁干活的那个人（机主）名下好发；只在别的账号名下的图照旧 `unknown_attachment`。
#[tokio::test]
async fn an_extension_working_for_the_owner_finds_its_own_upload() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "helper");
    let picture = png(3, 3);
    let p = ContentHash::of(&picture).as_str().to_string();
    let elsewhere = png(5, 5);
    let owners = png(4, 4);
    let o = Blobs::new(home.root.blobs(&alice()))
        .put(&owners)
        .expect("存得进")
        .as_str()
        .to_string();
    let carol = AccountId::parse("carol").expect("合写法");
    let e = Blobs::new(home.root.blobs(&carol))
        .put(&elsewhere)
        .expect("存得进")
        .as_str()
        .to_string();
    install_serving(
        &home,
        "helper",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!(
                r#"ask:blob.put:{{"data":"{}","name":"a.png"}}"#,
                STANDARD.encode(&picture)
            ),
            &format!(
                r#"ask:model.call:{{"purpose":"look","messages":[{{"role":"user","text":"看","images":["{p}"]}}]}}"#
            ),
            &format!(
                r#"ask:model.call:{{"purpose":"look","messages":[{{"role":"user","text":"看","images":["{e}"]}}]}}"#
            ),
            &format!(
                r#"ask:model.call:{{"purpose":"look","messages":[{{"role":"user","text":"看","images":["{o}"]}}]}}"#
            ),
            "wait",
        ]),
        "always",
        false,
    );
    let script = Script::new([]);
    let _core = served_core(&home, &script);
    let read = replies(&path, 5).await;
    let helper = AccountId::parse("helper").expect("合写法");
    assert!(holds(&home, &helper, &p), "存在扩展自己名下：{read:?}");
    // 这个核心没配模型：图先找到了，才轮到找模型。
    assert_eq!(
        read[2]["error"]["data"]["reason"], "no_model",
        "自己存的图找得到：{:?}",
        read[2]
    );
    assert!(holds(&home, &alice(), &p), "拷进了替谁干活的那个人名下");
    assert_eq!(
        read[3]["error"]["data"]["reason"], "unknown_attachment",
        "别人名下的图照旧找不到：{:?}",
        read[3]
    );
    assert_eq!(
        read[4]["error"]["data"]["reason"], "no_model",
        "自己名下没有、替谁干活的那个人名下有的照样找得到：{:?}",
        read[4]
    );
}

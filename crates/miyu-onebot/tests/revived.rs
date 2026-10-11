//! 重启那一刻不丢话（施工 O-32 补，`onebot.md` 第一条「群里怎么叫她」第 1 条、「施工时定的」第 236 条）：测试当核心跑
//! `serve`。核心带 `after` 订阅时，回应以前就可能推来序号大于 `upto` 的新事件（`relay` 先放已经到了的推送）；这时桥还不知道
//! 起来时列出来的会话发到哪（补完了照日志认），她新说的那一句也照补发那条路发。群里、白名单成员的私聊都不丢。补完了也认不出
//! 发到哪的，她之后说的发不出去，记一行 `WARN reply dropped why=unknown_peer`，不悄悄丢。
//!
//! 不靠时机：测试照核心的样子，先推补来的（序号 1 到 3），再推她新说的一句（序号 4），最后才回订阅的回应（`upto` 是 3）。

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use miyu_kernel::time::Timestamp;
use miyu_store::root::DataRoot;

use crate::support::group::*;
use crate::support::ports::on_free_port;
use crate::support::spawning::{Served, served_listening};
use crate::support::*;

/// 群号。
const GROUP: i64 = 671;

/// 白名单成员。
const JIE: i64 = 20003;

/// 起来时列出来的那一个场所会话。
const SESSION: &str = "0199d0c4-1a2b-7c3d-8e4f-5a6b7c8d9e0f";

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 她新说的那一句。
const WORDS: &str = "接着说的。";

/// 此刻，写成日志里的样子：补发只补期限以内的。
fn now() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("在 1970 年以后")
        .as_millis();
    let millis = i64::try_from(millis).expect("放得下");
    Timestamp::from_unix_millis(millis)
        .expect("在范围里")
        .to_string()
}

/// 场所 `venue` 的会话日志的头四条：补来的 `session.created`、号是 `user` 的人说的一句（机器人号是 [`BOT`] 的命令编号）、
/// 照它开的一轮；序号 4 是她在这一轮新说的 [`WORDS`]。
fn log(venue: &str, user: i64) -> Vec<Value> {
    let at = now();
    let policy = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let created = json!({"seq": 1, "at": at, "kind": "session.created", "by": {"kind": "kernel"},
        "body": {"owner": "onebot", "venue": venue, "policy": policy,
            "permission": {"level": "workspace", "read_only": false}}});
    let said = json!({"seq": 2, "at": at, "kind": "message.user",
        "by": {"kind": "external", "venue": venue, "id": format!("qq:{user}")},
        "cause": format!("qq:{BOT}:2:{TIME}"),
        "body": {"blocks": [{"type": "text", "text": "在吗"}], "venue": {"msg": "2", "ambient": true}}});
    let started = json!({"seq": 3, "at": at, "kind": "turn.started", "turn": 3, "by": {"kind": "kernel"},
        "body": {"trigger": 2, "triggers": [2]}});
    let words = json!({"seq": 4, "at": at, "kind": "message.assistant", "turn": 3,
        "by": {"kind": "model", "endpoint": "deepseek", "model": "deepseek-v4"},
        "body": {"blocks": [{"type": "text", "text": WORDS}], "seen": 1}});
    vec![created, said, started, words]
}

/// 照核心的样子答桥起来时的几问：`venue.sessions` 列出场所 `venue` 的会话 [`SESSION`]；桥订阅它（`after: 0`）时先推 `log`
/// 的每一条，再回 `upto: 3`。
async fn answer_as_core(served: &mut Served, venue: &str, log: Vec<Value>) {
    let listed = served.asked("venue.sessions").await;
    let sessions = json!([{"session": SESSION, "venue": venue}]);
    served
        .send(&json!({"jsonrpc": "2.0", "id": listed["id"], "result": {"sessions": sessions}}))
        .await;
    let subscribe = served.asked("subscribe").await;
    assert_eq!(subscribe["params"]["after"], 0, "{subscribe}");
    for event in log {
        push(served, event).await;
    }
    let upto =
        json!({"jsonrpc": "2.0", "id": subscribe["id"], "result": {"limits": {}, "upto": 3}});
    served.send(&upto).await;
}

/// 照核心的样子推会话 [`SESSION`] 的一条事件 `event`。
async fn push(served: &mut Served, event: Value) {
    let pushed = json!({"jsonrpc": "2.0", "method": "event",
        "params": {"session": SESSION, "stream": "events", "event": event}});
    served.send(&pushed).await;
}

/// 等桥入队她的话（`events.append`，正文是 [`WORDS`]），回序号 5。
async fn appended(served: &mut Served) {
    let appended = served.asked("events.append").await;
    assert_eq!(appended["params"]["body"]["text"], WORDS, "{appended}");
    served
        .send(&json!({"jsonrpc": "2.0", "id": appended["id"], "result": {"seq": 5}}))
        .await;
}

/// 跑 `serve`（握手交的配置照 `config`，再加上端口和令牌），假 NapCat 连上，再照 [`answer_as_core`] 答：交回桥、假 NapCat、
/// 数据根和要删的临时目录。
async fn revived(
    config: Value,
    venue: &str,
    log: Vec<Value>,
) -> (Served, Answering, DataRoot, PathBuf) {
    let (dir, root) = temp_root();
    let (mut served, listen) = on_free_port(async |listen| {
        let mut config = config.clone();
        config["onebot.listen"] = json!(listen);
        config["onebot.token"] = json!(TOKEN);
        Ok((served_listening(&root, config, listen).await?, listen))
    })
    .await;
    let napcat = admin_napcat(listen).await.answering(MEMBERS);
    answer_as_core(&mut served, venue, log).await;
    (served, napcat, root, dir)
}

/// 等到运行日志里有 `wanted`。
async fn until_log(root: &DataRoot, wanted: &str) {
    within(&format!("运行日志里有 {wanted}"), async {
        while !run_log(root).contains(wanted) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
}

/// 停下桥，删掉临时目录。
async fn done(mut served: Served, dir: PathBuf) {
    drop(served.stdin.take());
    within("桥退出", served.child.wait()).await.expect("等得到");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn her_new_words_pushed_before_the_subscribe_reply_reach_the_group() {
    let venue = format!("qq:group:{GROUP}");
    let (mut served, mut napcat, _, dir) = revived(json!({}), &venue, log(&venue, STRANGER)).await;
    appended(&mut served).await;
    let message = napcat.group_message(GROUP).await;
    assert_eq!(message.last().expect("有段")["data"]["text"], WORDS);
    assert!(napcat.pending().is_none(), "只发一遍");
    done(served, dir).await;
}

#[tokio::test]
async fn her_new_words_pushed_before_the_subscribe_reply_reach_a_private_chat() {
    let venue = format!("qq:private:{JIE}");
    let config = json!({"onebot.whitelist": [format!("qq:{JIE}")]});
    let (mut served, mut napcat, _, dir) = revived(config, &venue, log(&venue, JIE)).await;
    appended(&mut served).await;
    let action = napcat.action().await;
    assert_eq!(action["action"], "send_private_msg", "{action}");
    assert_eq!(action["params"]["user_id"], JIE, "{action}");
    assert_eq!(
        action["params"]["message"][0]["data"]["text"], WORDS,
        "{action}"
    );
    assert!(napcat.pending().is_none(), "只发一遍");
    done(served, dir).await;
}

#[tokio::test]
async fn her_words_with_nowhere_to_go_are_logged_not_dropped_silently() {
    // 日志里认不出机器人号（人说的那一句的命令编号不是 `qq:<号>:…`）：补完了还不知道发到哪，之后她说的发不出去，记一行。
    for (venue, config) in [
        (format!("qq:group:{GROUP}"), json!({})),
        (
            format!("qq:private:{JIE}"),
            json!({"onebot.whitelist": [format!("qq:{JIE}")]}),
        ),
    ] {
        let mut log = log(&venue, JIE);
        log[1]["cause"] = json!("cmd-2");
        let words = log.pop().expect("有她说的");
        let (mut served, _napcat, root, dir) = revived(config, &venue, log).await;
        until_log(&root, "bot not known yet").await;
        push(&mut served, words).await;
        until_log(&root, "why=unknown_peer").await;
        let line = run_log(&root)
            .lines()
            .find(|line| line.contains("why=unknown_peer"))
            .expect("有这一行")
            .to_string();
        assert!(line.contains("WARN"), "{line}");
        assert!(line.contains(SESSION), "{line}");
        assert!(!line.contains(WORDS), "不记原文：{line}");
        done(served, dir).await;
    }
}

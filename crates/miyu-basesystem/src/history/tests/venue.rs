//! 补拉回来的群消息（施工 O-40，`docs/blueprint/tools/history.md`「时刻」）：平台给了时刻的照它显示、照它筛，序号照记下的先后。

use super::*;

/// 第 1 条 05:01 记下；第 2 条 05:05 才补回来，平台说是 04:30 发的。
fn backfilled() -> Vec<Vec<Event>> {
    let said = |seq: u64, at: &str, text: &str, venue: Value| {
        event(
            seq,
            at,
            "message.user",
            None,
            alice(),
            json!({"blocks": [{"type": "text", "text": text}], "venue": venue}),
        )
    };
    vec![vec![
        said(
            1,
            "2026-09-29T05:01:00.000Z",
            "早",
            json!({"msg": "8801", "ambient": true}),
        ),
        said(
            2,
            "2026-09-29T05:05:00.000Z",
            "断线时说的",
            json!({"msg": "8799", "ambient": true, "backfill": true, "sent": "2026-09-29T04:30:00.000Z"}),
        ),
    ]]
}

#[tokio::test]
async fn a_backfilled_message_shows_when_it_was_sent() {
    let done = run_on(backfilled(), json!({})).await;
    assert_eq!(
        text(&done),
        "#1 2026-09-29 14:01 user\n早\n\n#2 2026-09-29 13:30 user\n断线时说的\n"
    );
    // 筛的时刻也照平台给的。
    let early = run_on(backfilled(), json!({"until": "2026-09-29 13:45"})).await;
    assert_eq!(seqs(&early), [2]);
}

/// 群里人说的话（施工 O-42）：「谁」那一格照群里那一行写发的人：名字；这一条 `show_ids` 的带 `id=`；管理员、主人标出来；
/// 没名字的写平台身份；名字照模板的规矩转义成一行。按 `user` 筛照样算人说的。
#[tokio::test]
async fn a_venue_message_names_who_said_it() {
    let member = |id: &str, role: &str, account: Option<&str>| {
        let mut by = json!({"kind": "external", "venue": "qq:group:1", "id": id, "role": role});
        if let Some(account) = account {
            by["account"] = json!(account);
        }
        by
    };
    let said = |seq: u64, by: Value, venue: Value| {
        event(
            seq,
            "2026-10-21T03:35:00.000Z",
            "message.user",
            None,
            by,
            json!({"blocks": [{"type": "text", "text": "bitwarden"}], "venue": venue}),
        )
    };
    let log = vec![vec![
        said(
            1,
            member("qq:20017", "member", None),
            json!({"msg": "1", "name": "小林"}),
        ),
        said(
            2,
            member("qq:20018", "manager", None),
            json!({"msg": "2", "name": "老王", "show_ids": true}),
        ),
        said(
            3,
            member("qq:1", "member", Some("admin")),
            json!({"msg": "3", "name": "主人"}),
        ),
        said(4, member("qq:20019", "member", None), json!({"msg": "4"})),
        said(
            5,
            member("qq:20020", "member", None),
            json!({"msg": "5", "name": "a\nb"}),
        ),
    ]];
    let done = run_on(log.clone(), json!({})).await;
    let b = '\\';
    assert_eq!(
        text(&done),
        format!(
            "#1 2026-10-21 12:35 小林\nbitwarden\n\n#2 2026-10-21 12:35 老王 (id=qq:20018, manager)\nbitwarden\n\n\
             #3 2026-10-21 12:35 主人 (owner)\nbitwarden\n\n#4 2026-10-21 12:35 qq:20019\nbitwarden\n\n\
             #5 2026-10-21 12:35 a{b}nb\nbitwarden\n"
        )
    );
    let users = run_on(log, json!({"by": "user"})).await;
    assert_eq!(seqs(&users), [1, 2, 3, 4, 5], "按 user 筛照样算人说的");
}

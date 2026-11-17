//! 人生会话（施工 LF-5 中，`store/index.md`「一行记什么」的 `life`）：照日志里的 `life.started` 派生，写进表里存成整数、读得回来；
//! 别的会话是「不是」；表里存了认不得的数的报这一行读不懂。

use super::*;

fn started(seq: u64) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:01.000Z","kind":"life.started","by":{{"kind":"person"}},"cause":"c1/life","body":{{}}}}"#
    ))
}

#[test]
fn a_life_session_is_known_from_its_log_and_survives_the_table() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    assert_eq!(listed.life, Life::Not, "刚造的不是");
    listed.see(&started(2));
    assert_eq!(listed.life, Life::Started);
    listed.see(&renamed(3, "早上", false));
    assert_eq!(listed.life, Life::Started, "别的事件不动它");
    listed.mark = mark(300, 4);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写进去读得回来");

    let mut plain = Row::new(id(), &created()).unwrap();
    plain.mark = mark(100, 2);
    index.remove(&id()).unwrap();
    assert!(index.put(&plain, None).unwrap());
    assert_eq!(row(&index).unwrap().life, Life::Not, "别的会话不是");
}

#[test]
fn a_life_number_the_index_does_not_know_is_an_unreadable_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    listed.see(&started(2));
    listed.mark = mark(300, 3);
    assert!(index.put(&listed, None).unwrap());
    drop(index);
    let path = scratch.rooted().join("index").join(FILE);
    let db = Connection::open(&path).unwrap();
    db.execute("UPDATE sessions SET life = 7", []).unwrap();
    drop(db);
    let (index, _) = open(&scratch);
    let error = index.rows().unwrap_err();
    assert!(error.to_string().contains("life"), "{error}");
}

fn ended(seq: u64) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:02.000Z","kind":"life.ended","by":{{"kind":"person"}},"cause":"c1/end","body":{{}}}}"#
    ))
}

/// 施工 LF-4 上：结束了的照 `life.ended` 派生成第三种，存成 2、读得回来。
#[test]
fn an_ended_life_is_the_third_kind_and_survives_the_table() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    listed.see(&started(2));
    listed.see(&ended(3));
    assert_eq!(listed.life, Life::Ended);
    assert_eq!(Life::Ended.number(), 2);
    assert_eq!(Life::from_number(2), Some(Life::Ended));
    listed.mark = mark(400, 4);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写进去读得回来");
}

/// 列会话照日志补的那一行写回去，人生那一格跟着写（施工 SX-1 顺手修：以前换整行的时候只写到第 20 格，后加的几格停在旧的）。
#[test]
fn a_caught_up_row_writes_its_life_back() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    listed.mark = mark(100, 2);
    assert!(index.put(&listed, None).unwrap());
    let was = listed.mark;
    listed.see(&started(2));
    listed.see(&ended(3));
    listed.mark = mark(400, 4);
    assert!(index.put(&listed, Some(&was)).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "补上的几格都写回去了");
}

/// 人生循环交进来的那段话（施工 LF-15）不当第一句话：它的正文是推演方法，人生会话刚开时列表上那一行原来是「Live your own
/// life here…」。循环交进来的（`/loop` 的那句是人写的）照旧算；人后来在人生会话里说的第一句照旧算。
#[test]
fn a_life_wake_is_not_the_preview() {
    let wake = |seq: u64, kind: &str| {
        event(&format!(
            r#"{{"seq":{seq},"at":"2026-09-25T09:00:00.000Z","kind":"message.user","by":{{"kind":"schedule","schedule":"t1","loop":"{kind}"}},"cause":"c{seq}","body":{{"blocks":[{{"type":"text","text":"Live your own life here, one moment at a time."}}]}}}}"#
        ))
    };
    let mut row = Row::new(id(), &created()).unwrap();
    row.see(&wake(2, "life"));
    assert_eq!(row.preview, "", "人生循环交进来的不算");
    row.see(&said(3, r#"[{"type":"text","text":"在吗"}]"#));
    assert_eq!(row.preview, "在吗", "人说的第一句照旧算");

    let mut row = Row::new(id(), &created()).unwrap();
    row.see(&wake(2, "dynamic"));
    assert_eq!(
        row.preview, "Live your own life here, one moment at a time.",
        "循环交进来的照旧算"
    );
}

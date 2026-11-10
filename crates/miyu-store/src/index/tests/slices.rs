//! 只读一截、只读认父子的两格（施工 YC-13）：带上限的列会话照它们读，不把整张表解出来。

use super::*;

fn nth(n: u64) -> SessionId {
    SessionId::parse(&format!("0192f3a0-{n:04x}-7abc-8def-001122334455")).unwrap()
}

/// 表里放 1 到 5 号五个会话：3 号是 2 号的子会话。
fn five(index: &SessionIndex) {
    for n in 1..=5 {
        let mut row = Row::new(nth(n), &created()).unwrap();
        row.parent = (n == 3).then(|| nth(2));
        row.tokens = n * 10;
        assert!(index.put(&row, None).unwrap());
    }
}

#[test]
fn rows_between_gives_the_rows_in_the_span_both_ends_included() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    five(&index);
    let all = index.rows().unwrap();
    let ids = |rows: &BTreeMap<SessionId, Row>| rows.keys().cloned().collect::<Vec<_>>();
    let span = index.rows_between(&nth(2), &nth(4)).unwrap();
    assert_eq!(ids(&span), [nth(2), nth(3), nth(4)]);
    for (id, row) in &span {
        assert_eq!(Some(row), all.get(id), "和读全部的是同一行");
    }
    assert_eq!(
        ids(&index.rows_between(&nth(5), &nth(5)).unwrap()),
        [nth(5)]
    );
    assert_eq!(index.rows_between(&nth(0), &nth(9)).unwrap(), all);
    assert!(index.rows_between(&nth(6), &nth(9)).unwrap().is_empty());
    assert!(
        index.rows_between(&nth(4), &nth(2)).unwrap().is_empty(),
        "两头反了的没有"
    );
}

#[test]
fn links_are_the_parent_and_tokens_of_every_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    assert!(index.links().unwrap().is_empty());
    five(&index);
    let links = index.links().unwrap();
    let rows = index.rows().unwrap();
    assert_eq!(links.len(), 5);
    for (id, row) in &rows {
        let link = Link {
            parent: row.parent.clone(),
            tokens: row.tokens,
        };
        assert_eq!(links.get(id), Some(&link));
    }
    assert_eq!(links[&nth(3)].parent, Some(nth(2)));
    // 读不懂的父会话照实报，不当没有。
    Connection::open(scratch.rooted().join("index").join(FILE))
        .unwrap()
        .execute("UPDATE sessions SET parent = 'not a session'", [])
        .unwrap();
    assert!(index.links().is_err());
    assert!(index.rows_between(&nth(1), &nth(1)).is_err());
}

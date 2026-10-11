//! 合并转发那几行的字（施工 O-34）：出厂的三句交给组装器时去掉行尾的换行、两份照模板读；快照字节读得回来；O-34 以前造的没有
//! 这一格，读成没有、写回去一个字节不变；模板写错了、字段不对的造不出策略。

use std::collections::BTreeMap;

use super::*;
use crate::test_support::engineer;

#[test]
fn the_factory_texts_reach_the_assembler_without_the_line_end() {
    let made = engineer();
    let texts = made.forward().unwrap().expect("O-34 起造的有");
    assert_eq!(texts.label, "forward:");
    let number = BTreeMap::from([("number", "2")]);
    assert_eq!(texts.numbered.render(&number).unwrap(), "forward #2:");
    let count = BTreeMap::from([("count", "12")]);
    assert_eq!(texts.more.render(&count).unwrap(), "(12 more not shown)");
    assert_eq!(Snapshot::from_bytes(&made.to_bytes()).unwrap(), made);
    assert!(made.policy().is_ok());
}

#[test]
fn a_snapshot_made_before_has_none_and_keeps_its_bytes() {
    let mut older = engineer();
    older.core.forward = None;
    let bytes = older.to_bytes();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("\"forward\"")
    );
    let read = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(read.core.forward, None);
    assert_eq!(read.to_bytes(), bytes, "读进来再写出去一个字节不变");
    assert!(read.forward().unwrap().is_none());
    assert!(read.policy().is_ok());
}

#[test]
fn a_broken_template_builds_no_policy() {
    for (numbered, more) in [
        ("forward #{number:\n", "({count} more not shown)\n"),
        ("forward #{n}:\n", "({count} more not shown)\n"),
        ("forward #{number}:\n", "({left} more not shown)\n"),
    ] {
        let mut made = engineer();
        made.core.forward = Some(ForwardTexts {
            label: "forward:\n".to_string(),
            numbered: numbered.to_string(),
            more: more.to_string(),
        });
        assert!(
            matches!(
                made.policy(),
                Err(BuildError::Texts {
                    which: "forward",
                    ..
                })
            ),
            "{numbered} {more}"
        );
    }
}

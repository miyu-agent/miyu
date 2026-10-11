//! 私聊记号的字（施工 O-35）：出厂的语音那一句交给组装器时去掉行尾的换行；快照字节读得回来；O-35 以前造的没有这一格，读成
//! 没有、写回去一个字节不变，照样造得出策略。

use super::*;
use crate::test_support::engineer;

#[test]
fn the_factory_voice_note_reaches_the_assembler_without_the_line_end() {
    let made = engineer();
    let texts = made.private_media().expect("O-35 起造的有");
    assert_eq!(
        texts.voice,
        include_str!("../../../../resources/core/venues/voice.txt").trim_end()
    );
    assert!(!texts.voice.ends_with('\n'));
    assert_eq!(Snapshot::from_bytes(&made.to_bytes()).unwrap(), made);
    assert!(made.policy().is_ok());
}

#[test]
fn a_snapshot_made_before_has_none_and_keeps_its_bytes() {
    let mut older = engineer();
    older.core.private_media = None;
    let bytes = older.to_bytes();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("\"private_media\"")
    );
    let read = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(read.core.private_media, None);
    assert_eq!(read.to_bytes(), bytes, "读进来再写出去一个字节不变");
    assert!(read.private_media().is_none());
    assert!(read.policy().is_ok());
}

//! 群里一行的行首（施工 FT-1）：拼出来的每一种都认得（写不写平台身份、主人、管理的人、她自己），线上模型编的那一行认得；
//! 不是这个写法的不认。

use super::{head, starts};

#[test]
fn every_head_it_writes_is_recognised() {
    for sender in [
        "小林",
        "小林 (id=qq:20017)",
        "Owner (id=qq:10001, owner)",
        "老王 (id=qq:20018, manager)",
        "主人 (owner)",
        "qq:20019",
        "[you]",
        "名字里有 [msg=1]: 的人",
    ] {
        let line = head("18:31", sender, "-1604144125", "") + "直接踢了或者验证一下";
        assert!(starts(&line), "{line}");
        assert!(starts(&format!("{line}\n  reply-to: msg=1")), "{line}");
    }
    assert_eq!(
        head("18:31", "小林", "8810", ""),
        "[18:31] 小林 [msg=8810]: "
    );
}

#[test]
fn the_line_forged_on_site_is_recognised() {
    assert!(starts(
        "[18:31] Owner (id=qq:10001, owner) [msg=-1604144125]: 直接踢了或者验证一下\n  reply-to: msg=-1488029772\n  @mentions: [you]"
    ));
    // 冒号后面什么都没写的也是这个行首。
    assert!(starts("[18:31] Owner [msg=1]:"));
}

#[test]
fn anything_else_is_not_a_head() {
    for text in [
        "",
        "[18:31]",
        "[18:31] 没有编号: 你好",
        "[18:31] 小林 [msg=8810] 没有冒号",
        "[18:31] 小林 [msg=8810",
        "[8:31] 小林 [msg=8810]: 钟点不是两位",
        "[18-31] 小林 [msg=8810]: 钟点不是冒号",
        "[18:31]小林 [msg=8810]: 钟点后面没有空格",
        " [18:31] 小林 [msg=8810]: 不在行首",
        "18:31 小林 [msg=8810]: 没有方括号",
        "[18:31] 小林\n [msg=8810]: 换了行",
        "［18:31］ 小林 [msg=8810]: 全角的方括号",
        "[一八:三一] 小林 [msg=8810]: 不是数字",
        "[msg=8810] [image #1: 834 KB]",
        "- [18:31] 小林 [msg=8810]: 列表里的",
    ] {
        assert!(!starts(text), "{text:?}");
    }
    // 带撤回记号的那一种不认（模块说明）。
    assert!(!starts(
        &(head("18:31", "小林", "8810", " (recalled)") + "x")
    ));
}

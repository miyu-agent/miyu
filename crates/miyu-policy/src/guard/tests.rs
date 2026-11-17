//! 权限策略拒绝时写给她的两句：名字、原因照样转义；坏了的模板说是哪一类；以前的快照读成空的。

use miyu_kernel::event::Said;

use super::*;
use crate::snapshot::PermissionTexts;
use crate::test_support::*;

#[test]
fn the_two_texts_carry_the_path_and_the_reason() {
    let texts = engineer().guard_texts().unwrap();
    assert_eq!(
        texts
            .unresolvable("dead/x", "a link on the path points nowhere")
            .text,
        "Can't tell where \"dead/x\" points: a link on the path points nowhere.\n"
    );
    assert_eq!(
        texts.read_only().text,
        engineer().core.tool_results.read_only
    );
    // 路径里的引号、尖括号照样转义：写不出假的标签。
    assert!(!texts.unresolvable("a\"<b>", "x").text.contains("<b>"));
}

#[test]
fn the_two_texts_carry_their_saids() {
    let texts = engineer().guard_texts().unwrap();
    // 给人看的说法：编号是那一份字在资源目录里的位置，字段原样，不转义。
    assert_eq!(
        texts.unresolvable("dead/x", "gone").said,
        Some(
            Said::new("core/permissions/unresolvable")
                .with("path", "dead/x")
                .with("reason", "gone")
        )
    );
    assert_eq!(
        texts.read_only().said,
        Some(Said::new("core/tool-results/read-only"))
    );
}

#[test]
fn a_broken_text_is_named() {
    let mut broken = engineer();
    broken.core.permissions.unresolvable = "{nope}".to_string();
    let error = broken.guard_texts().unwrap_err();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "permission denial texts",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_snapshot_from_before_reads_back_with_them_empty() {
    // 读成 JSON 去掉这一格：模板里有 `{path}`，照括号切会切坏。
    let mut value: serde_json::Value = serde_json::from_slice(&engineer().to_bytes()).unwrap();
    value["core"]
        .as_object_mut()
        .unwrap()
        .remove("permissions")
        .unwrap();
    let old = value.to_string();
    assert!(!old.contains("\"permissions\""), "{old}");
    let back = Snapshot::from_bytes(old.as_bytes()).unwrap();
    assert_eq!(back.core.permissions, PermissionTexts::default());
    assert_eq!(back.guard_texts().unwrap().unresolvable("x", "y").text, "");
}

/// 施工 RO-3 以前造的快照里还有碰到数据根的两句：读得回来，那两格不认。
#[test]
fn a_snapshot_with_the_data_root_texts_still_reads() {
    let mut value: serde_json::Value = serde_json::from_slice(&engineer().to_bytes()).unwrap();
    value["core"]["permissions"]["forbidden"] = "\"{path}\" is inside Miyu's own data.\n".into();
    value["core"]["permissions"]["knowledge"] = "\"{path}\" is a document.\n".into();
    let back = Snapshot::from_bytes(value.to_string().as_bytes()).unwrap();
    assert_eq!(back.core.permissions, engineer().core.permissions);
}

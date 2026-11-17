//! `[[purpose]]`（施工 UP-3）：读成编号、名字，照写的先后；编号不合用途的写法、少了、重了，少了名字的报 `bad_purpose`；不是扩展包
//! 的不能写；不另要能力。编号的写法同 `model.call` 的 `purpose`。

use super::purpose_written;
use crate::package::{Code, read};

/// 一份扩展包的清单，再接上 `rest`。
fn extension(rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bot\" }}\n\n[command]\nname = \"bot\"\nprogram = \"bot\"\nabout = {{ en = \"Bot\" }}\n\n[process]\ncapabilities = []\n\n{rest}"
    )
}

const ONE: &str =
    "[[purpose]]\nid = \"join-approval\"\nname = { zh = \"入群审核\", en = \"Join review\" }\n";

#[test]
fn purposes_read_their_id_and_name_in_order() {
    let two = format!("{ONE}\n[[purpose]]\nid = \"2nd\"\nname = {{ en = \"Second\" }}\n");
    let manifest = read(&extension(&two)).unwrap();
    let ids: Vec<&str> = manifest
        .purposes
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(ids, ["join-approval", "2nd"], "照写的先后；数字开头的也认");
    assert_eq!(
        manifest.purposes[0].name.get("zh").map(String::as_str),
        Some("入群审核")
    );
    assert!(
        read(&extension("")).unwrap().purposes.is_empty(),
        "没写的是空的"
    );
}

#[test]
fn the_written_form_is_model_call_s() {
    for good in ["a", "join-approval", "2nd", &"x".repeat(32)] {
        assert!(purpose_written(good), "{good}");
    }
    for bad in ["", "Judge", "a_b", "a b", "入群", &"x".repeat(33)] {
        assert!(!purpose_written(bad), "{bad}");
    }
}

#[test]
fn wrong_purposes_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成");
    for (text, detail) in [
        (
            ONE.replace("\"join-approval\"", "\"Join\""),
            "purpose.id \"Join\"",
        ),
        (ONE.replace("id = \"join-approval\"\n", ""), "purpose.id"),
        (format!("{ONE}\n{ONE}"), "purpose.join-approval"),
        (
            ONE.replace("name = { zh = \"入群审核\", en = \"Join review\" }\n", ""),
            "purpose.join-approval.name",
        ),
    ] {
        let problem = wrong(extension(&text));
        assert_eq!(problem.code, Code::BadPurpose, "{text}");
        assert_eq!(problem.detail, detail, "{text}");
    }
    assert_eq!(
        wrong(extension(&format!("{ONE}tier = \"lite\"\n"))).code,
        Code::UnknownKey,
        "不认识的格"
    );
    assert_eq!(
        wrong(extension("[purpose]\nid = \"judge\"\n")).code,
        Code::NotATable,
        "要写成 [[purpose]]"
    );
    let ui = format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[ui]\n\n{ONE}");
    assert_eq!(wrong(ui).code, Code::WrongKind, "只有扩展包能写");
}

//! `[[side]]`（施工 Q-3）：读成编号、档位、名字、说明，全编号是 `<包>/<编号>`；编号写错、重了、档位不是三档之一、少了名字或
//! 说明的报 `bad_side`；不是扩展包的不能写；不另要能力。

use crate::package::{Code, read};

/// 一份扩展包的清单，再接上 `rest`。
fn extension(rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bot\" }}\n\n[command]\nname = \"bot\"\nprogram = \"bot\"\nabout = {{ en = \"Bot\" }}\n\n[process]\ncapabilities = []\n\n{rest}"
    )
}

const ONE: &str = "[[side]]\nid = \"judge\"\ntier = \"lite\"\nname = { en = \"Reply judge\", zh = \"判断要不要回\" }\nsummary = { en = \"Decides whether to reply.\", zh = \"群里有人说话时判断要不要回\" }\n";

#[test]
fn a_side_request_reads_its_id_tier_name_and_summary() {
    let manifest = read(&extension(ONE)).unwrap();
    assert_eq!(manifest.side.len(), 1);
    let side = &manifest.side[0];
    assert_eq!(side.id, "judge");
    assert_eq!(side.tier, "lite");
    assert_eq!(
        side.name.get("zh").map(String::as_str),
        Some("判断要不要回")
    );
    assert_eq!(
        side.summary.get("en").map(String::as_str),
        Some("Decides whether to reply.")
    );
    assert_eq!(side.full_id("bot"), "bot/judge");
    let three = ["lite", "normal", "flagship"]
        .map(|tier| {
            ONE.replace("\"lite\"", &format!("\"{tier}\""))
                .replace("judge", tier)
        })
        .join("\n");
    let tiers: Vec<&str> = read(&extension(&three))
        .unwrap()
        .side
        .iter()
        .map(|side| side.tier)
        .collect();
    assert_eq!(
        tiers,
        ["lite", "normal", "flagship"],
        "三档都认，照写的先后"
    );
}

#[test]
fn wrong_side_requests_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成");
    for (text, detail) in [
        (ONE.replace("\"judge\"", "\"Judge\""), "side.id \"Judge\""),
        (ONE.replace("id = \"judge\"\n", ""), "side.id"),
        (format!("{ONE}\n{ONE}"), "side.judge"),
        (ONE.replace("\"lite\"", "\"standard\""), "side.judge.tier"),
        (ONE.replace("tier = \"lite\"\n", ""), "side.judge.tier"),
        (
            ONE.replace("name = { en = \"Reply judge\", zh = \"判断要不要回\" }\n", ""),
            "side.judge.name",
        ),
        (
            ONE.replace(
                "summary = { en = \"Decides whether to reply.\", zh = \"群里有人说话时判断要不要回\" }\n",
                "",
            ),
            "side.judge.summary",
        ),
    ] {
        let problem = wrong(extension(&text));
        assert_eq!(problem.code, Code::BadSide, "{text}");
        assert_eq!(problem.detail, detail, "{text}");
    }
    assert_eq!(
        wrong(extension(&format!("{ONE}model = \"x\"\n"))).code,
        Code::UnknownKey,
        "不认识的格"
    );
    assert_eq!(
        wrong(extension("[side]\nid = \"judge\"\n")).code,
        Code::NotATable,
        "要写成 [[side]]"
    );
    let ui = format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[ui]\n\n{ONE}");
    assert_eq!(wrong(ui).code, Code::WrongKind, "只有扩展包能写");
}

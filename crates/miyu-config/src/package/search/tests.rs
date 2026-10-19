//! `[[search]]`（施工 N-5）：读成编号、名字，下拉里的值是 `<包>/<编号>`；编号写错、重了、少了名字、没声明 `search` 能力的
//! 报 `bad_search`；不是扩展包的不能写。

use crate::package::{Code, read};

/// 一份扩展包的清单，能力是 `capabilities`，再接上 `rest`。
fn extension(capabilities: &str, rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Finder\" }}\n\n[command]\nname = \"finder\"\nprogram = \"finder\"\nabout = {{ en = \"Finder\" }}\n\n[process]\ncapabilities = [{capabilities}]\n\n{rest}"
    )
}

const ONE: &str = "[[search]]\nid = \"web\"\nname = { en = \"Finder\", zh = \"查找\" }\n";

#[test]
fn a_source_reads_its_id_and_name() {
    let manifest = read(&extension("\"search\"", ONE)).unwrap();
    assert_eq!(manifest.search.len(), 1);
    let source = &manifest.search[0];
    assert_eq!(source.id, "web");
    assert_eq!(source.name.get("zh").map(String::as_str), Some("查找"));
    assert_eq!(source.option("finder"), "finder/web");
}

#[test]
fn wrong_sources_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成").code;
    assert_eq!(
        wrong(extension("", ONE)),
        Code::BadSearch,
        "没声明 search 能力"
    );
    assert_eq!(
        wrong(extension("\"search\"", &ONE.replace("\"web\"", "\"Web\""))),
        Code::BadSearch
    );
    assert_eq!(
        wrong(extension("\"search\"", &format!("{ONE}\n{ONE}"))),
        Code::BadSearch,
        "重了"
    );
    assert_eq!(
        wrong(extension("\"search\"", "[[search]]\nid = \"web\"\n")),
        Code::BadSearch,
        "少了名字"
    );
    assert_eq!(
        wrong(extension("\"search\"", "[search]\nid = \"web\"\n")),
        Code::NotATable,
        "要写成 [[search]]"
    );
    let ui = format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[ui]\n\n{ONE}");
    assert_eq!(wrong(ui), Code::WrongKind, "只有扩展包能写");
}

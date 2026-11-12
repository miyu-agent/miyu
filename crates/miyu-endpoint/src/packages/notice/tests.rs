//! 「注意」怎么拼（施工 CM-1）：社区的、本机文件的各一句，几个包的写出是哪几个，官方的、出厂的不说，没有沙盒那一句。

use super::{Item, said};

/// 假的取句子：key 后面接参数，看得出用的是哪一句、写了哪几个。
fn sentence(key: &str, args: &[(&str, &str)]) -> Option<String> {
    match key {
        "packages/notice/separator" => Some("、".to_string()),
        _ => Some(match args {
            [] => key.to_string(),
            _ => format!("{key}:{}", args[0].1),
        }),
    }
}

fn item<'a>(id: &'a str, source: &'a str) -> Item<'a> {
    Item { id, source }
}

#[test]
fn a_community_package_and_a_local_file_each_get_their_own_sentence() {
    assert_eq!(
        said(&[item("weather", "community")], true, sentence),
        Some(vec!["packages/notice/unreviewed".to_string()])
    );
    assert_eq!(
        said(&[item("kit", "file")], true, sentence),
        Some(vec!["packages/notice/file".to_string()]),
        "本机文件的不说成社区包"
    );
}

#[test]
fn several_packages_are_named_community_first_then_local_files() {
    let items = [
        item("lib", "file"),
        item("app", "official"),
        item("weather", "community"),
        item("rec", "community"),
        item("net", "shipped"),
    ];
    assert_eq!(
        said(&items, false, sentence),
        Some(vec![
            "packages/notice/unreviewed/named:weather、rec".to_string(),
            "packages/notice/file/named:lib".to_string(),
        ])
    );
}

#[test]
fn official_and_shipped_packages_say_nothing() {
    let items = [
        item("app", "official"),
        item("net", "shipped"),
        item("x", "distro"),
    ];
    assert_eq!(said(&items, false, sentence), None);
    assert_eq!(said(&[], true, sentence), None);
}

#[test]
fn a_missing_sentence_is_not_papered_over() {
    let none = |_: &str, _: &[(&str, &str)]| None;
    assert_eq!(said(&[item("weather", "community")], true, none), None);
}

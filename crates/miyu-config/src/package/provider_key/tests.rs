//! `[provider_key]`（施工 PK-1）：读成目录里那一家的编号；写错的、和 `provider.key` 能力对不上的报 `bad_provider_key`；不是扩展包
//! 的不能写。

use crate::package::{Capability, Code, read};

/// 一份扩展包的清单，能力是 `capabilities`，再接上 `rest`。
fn extension(capabilities: &str, rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Voice\" }}\n\n[command]\nname = \"voice\"\nprogram = \"voice\"\nabout = {{ en = \"Voice\" }}\n\n[process]\ncapabilities = [{capabilities}]\n\n{rest}"
    )
}

const DECLARED: &str = "\"provider.key\"";

const ONE: &str = "[provider_key]\ncatalog = \"deepseek\"\n";

#[test]
fn provider_key_reads_the_catalog() {
    let manifest = read(&extension(DECLARED, ONE)).unwrap();
    let wanted = manifest.provider_key.expect("写了 [provider_key]");
    assert_eq!(wanted.catalog, "deepseek");
    assert!(
        manifest
            .process
            .expect("扩展包")
            .capabilities
            .contains(&Capability::ProviderKey)
    );
    let none = read(&extension("\"network\"", "")).unwrap();
    assert_eq!(none.provider_key, None, "没写的没有");
}

#[test]
fn wrong_provider_keys_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成").code;
    assert_eq!(
        wrong(extension("", ONE)),
        Code::BadProviderKey,
        "没声明 provider.key"
    );
    assert_eq!(
        wrong(extension(DECLARED, "")),
        Code::BadProviderKey,
        "声明了 provider.key 却没写要哪一家"
    );
    for (rest, why) in [
        ("[provider_key]\n", "少了 catalog"),
        ("[provider_key]\ncatalog = 1\n", "catalog 不是字"),
        (
            "[provider_key]\ncatalog = \"Deep Seek\"\n",
            "catalog 的写法",
        ),
    ] {
        assert_eq!(
            wrong(extension(DECLARED, rest)),
            Code::BadProviderKey,
            "{why}"
        );
    }
    assert_eq!(
        wrong(extension(DECLARED, &format!("{ONE}key = \"sk\"\n"))),
        Code::UnknownKey,
        "不认识的格"
    );
    assert_eq!(
        wrong(extension(
            DECLARED,
            "[[provider_key]]\ncatalog = \"deepseek\"\n"
        )),
        Code::NotATable,
        "只能写一张表"
    );
    let ui = format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[ui]\n\n{ONE}");
    assert_eq!(wrong(ui), Code::WrongKind, "只有扩展包能写");
}

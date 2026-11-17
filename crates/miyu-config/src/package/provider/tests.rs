//! `[providers.<编号>]`（施工 PV-4）：读成编号、名字、驱动和去掉名字的那一张；编号、驱动、地址写错的报 `bad_provider`，
//! 不认的格 `unknown_key`（`key` 也不收）；只有扩展包、`bundle` 能写。

use crate::package::{Code, read};

/// 一份扩展包的清单，声明一个驱动 `antigravity`，再接上 `rest`。
fn extension(rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Gravity\" }}\n\n[command]\nname = \"gravity\"\nprogram = \"gravity\"\nabout = {{ en = \"Gravity\" }}\n\n[process]\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"antigravity\"\n\n{rest}"
    )
}

const AGY: &str = "[providers.agy]\nname = { en = \"Antigravity\", zh = \"反重力\" }\ndriver = \"antigravity/antigravity\"\nlocal = true\n\n[providers.agy.models.\"gemini-3-pro\"]\nwindow = 1000000\n";

#[test]
fn a_provider_reads_its_id_name_driver_and_the_rest() {
    let manifest = read(&extension(AGY)).unwrap();
    assert_eq!(manifest.providers.len(), 1);
    let agy = &manifest.providers[0];
    assert_eq!(agy.id, "agy");
    assert_eq!(agy.name.get("zh").map(String::as_str), Some("反重力"));
    assert_eq!(agy.driver, "antigravity/antigravity");
    assert_eq!(agy.driver_package(), Some("antigravity"));
    assert!(agy.table.starts_with("[providers.agy]\n"), "{}", agy.table);
    assert!(
        !agy.table.contains("Antigravity"),
        "名字不进配置的那一张：{}",
        agy.table
    );
    assert!(agy.table.contains("local = true"), "{}", agy.table);
    assert!(
        agy.table.contains("window = 1000000"),
        "模型的资料照带：{}",
        agy.table
    );
    // 内置驱动也行，名字可以不写。
    let plain = read(&extension(
        "[providers.mine]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n",
    ))
    .unwrap();
    assert!(plain.providers[0].name.is_empty());
    assert_eq!(plain.providers[0].driver_package(), None);
    assert!(
        read(&extension("")).unwrap().providers.is_empty(),
        "没写的是空的"
    );
}

#[test]
fn wrong_providers_are_refused() {
    for (rest, code) in [
        (
            "[providers.Agy]\ndriver = \"openai-chat\"\n",
            Code::BadProvider,
        ),
        ("[providers.agy]\ndriver = \"gemini\"\n", Code::BadProvider),
        (
            "[providers.agy]\ndriver = \"antigravity/other\"\n",
            Code::BadProvider,
        ),
        (
            "[providers.agy]\ndriver = \"openai-chat\"\nbase_url = { env = \"X\" }\n",
            Code::BadProvider,
        ),
        ("[providers.agy]\nname = { en = \"A\" }\n", Code::MissingKey),
        (
            "[providers.agy]\ndriver = \"openai-chat\"\nkey = { secret = \"k\" }\n",
            Code::UnknownKey,
        ),
        ("[providers.agy]\ndriver = 3\n", Code::BadProvider),
        ("[providers]\nagy = 3\n", Code::NotATable),
    ] {
        let problem = read(&extension(rest)).unwrap_err();
        assert_eq!(problem.code, code, "{rest}：{problem}");
        assert!(problem.line.is_some(), "说是哪一行：{rest}");
    }
    assert_eq!(Code::BadProvider.as_str(), "bad_provider");
}

#[test]
fn only_extensions_and_bundles_carry_providers() {
    let bundle =
        "[package]\nname = { en = \"Mine\" }\n\n[providers.mine]\ndriver = \"openai-chat\"\n";
    assert_eq!(read(bundle).unwrap().providers.len(), 1);
    let theme = "[package]\nname = { en = \"Look\" }\n\n[theme.tui]\nfile = \"look.toml\"\n\n[providers.mine]\ndriver = \"openai-chat\"\n";
    assert_eq!(read(theme).unwrap_err().code, Code::WrongKind);
}

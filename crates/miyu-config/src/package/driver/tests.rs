//! `[[driver]]`（施工 MD-1，设计 66 第四节）：读成驱动名、缓存类型、要不要 key、看不看图，配置里写 `<包>/<驱动名>`；写错的、重了的、
//! 和 `model.driver` 能力对不上的报 `bad_driver`；不是扩展包的不能写。

use crate::package::{Code, DriverCache, DriverKey, read};

/// 一份扩展包的清单，能力是 `capabilities`，再接上 `rest`。
fn extension(capabilities: &str, rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Gravity\" }}\n\n[command]\nname = \"gravity\"\nprogram = \"gravity\"\nabout = {{ en = \"Gravity\" }}\n\n[process]\ncapabilities = [{capabilities}]\n\n{rest}"
    )
}

const ONE: &str =
    "[[driver]]\nname = \"antigravity\"\ncache = \"contract\"\nkey = \"none\"\nimages = true\n";

#[test]
fn a_driver_reads_its_name_cache_key_and_images() {
    let manifest = read(&extension("\"model.driver\"", ONE)).unwrap();
    assert_eq!(manifest.drivers.len(), 1);
    let driver = &manifest.drivers[0];
    assert_eq!(driver.name, "antigravity");
    assert_eq!(driver.cache, DriverCache::Contract);
    assert_eq!(driver.key, DriverKey::None);
    assert!(driver.images);
    assert_eq!(
        driver.reference("pi-antigravity"),
        "pi-antigravity/antigravity"
    );
    // 不写的：尽力而为的缓存、key 可有可无、不看图。
    let plain = read(&extension(
        "\"model.driver\"",
        "[[driver]]\nname = \"plain\"\n",
    ))
    .unwrap();
    let driver = &plain.drivers[0];
    assert_eq!(
        (driver.cache, driver.key, driver.images),
        (DriverCache::BestEffort, DriverKey::Optional, false)
    );
}

#[test]
fn wrong_drivers_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成").code;
    let declared = "\"model.driver\"";
    assert_eq!(
        wrong(extension("", ONE)),
        Code::BadDriver,
        "没声明 model.driver"
    );
    assert_eq!(
        wrong(extension(declared, "")),
        Code::BadDriver,
        "声明了 model.driver 却一个驱动都没写"
    );
    for (from, to, why) in [
        ("\"antigravity\"", "\"Anti/Gravity\"", "驱动名的写法"),
        ("cache = \"contract\"", "cache = \"always\"", "缓存类型"),
        ("key = \"none\"", "key = \"maybe\"", "要不要 key"),
        ("images = true", "images = \"yes\"", "看不看图"),
    ] {
        assert_eq!(
            wrong(extension(declared, &ONE.replace(from, to))),
            Code::BadDriver,
            "{why}"
        );
    }
    assert_eq!(
        wrong(extension(declared, &format!("{ONE}\n{ONE}"))),
        Code::BadDriver,
        "重了"
    );
    assert_eq!(
        wrong(extension(declared, "[[driver]]\ncache = \"none\"\n")),
        Code::BadDriver,
        "少了驱动名"
    );
    assert_eq!(
        wrong(extension(declared, "[driver]\nname = \"x\"\n")),
        Code::NotATable,
        "要写成 [[driver]]"
    );
    assert_eq!(
        wrong(extension(declared, &format!("{ONE}color = \"red\"\n"))),
        Code::UnknownKey,
        "不认识的格"
    );
    let ui = format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[ui]\n\n{ONE}");
    assert_eq!(wrong(ui), Code::WrongKind, "只有扩展包能写");
}

#[test]
fn hosted_tool_names_read_and_default_to_none() {
    let declared = "\"model.driver\"";
    let manifest = read(&extension(
        declared,
        &format!("{ONE}hosted = [\"search_web\", \"read_url_content\"]\n"),
    ))
    .unwrap();
    assert_eq!(
        manifest.drivers[0].hosted,
        ["search_web", "read_url_content"]
    );
    assert!(
        read(&extension(declared, ONE)).unwrap().drivers[0]
            .hosted
            .is_empty(),
        "不写是空的"
    );
    let long = "x".repeat(64);
    let fine = read(&extension(
        declared,
        &format!("{ONE}hosted = [\"{long}\"]\n"),
    ))
    .unwrap();
    assert_eq!(fine.drivers[0].hosted, [long], "64 个字节正好");
}

#[test]
fn wrong_hosted_tool_names_are_refused_on_their_line() {
    let declared = "\"model.driver\"";
    let long = "x".repeat(65);
    for (hosted, why) in [
        ("[\"\"]", "空名字"),
        (&*format!("[\"{long}\"]"), "太长"),
        ("[\"a\", \"b\", \"a\"]", "重复"),
        ("[\"tab\\tname\"]", "控制字符"),
        ("\"search_web\"", "不是数组"),
        ("[\"a\", 1]", "不是字符串数组"),
    ] {
        // 驱动这一段从第 13 行起（清单的前面几段占了 12 行）。
        let problem =
            read(&extension(declared, &format!("{ONE}hosted = {hosted}\n"))).expect_err(why);
        assert_eq!(problem.code, Code::BadDriver, "{why}");
        assert_eq!(problem.detail, "driver.antigravity.hosted", "{why}");
        assert_eq!(problem.line, Some(13), "{why}");
    }
}

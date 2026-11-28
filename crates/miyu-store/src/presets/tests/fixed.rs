//! 定死的预设（施工 FP-1）：出厂的那一份写了 `[preset] fixed = true` 的只认出厂的，家目录那一层的改动不读、列进
//! [`Presets::ignored`]；别的层写不得 `fixed`。

use super::*;

const FIXED: &str = "[preset]\nname = \"Everything\"\nunlisted = \"on\"\nfixed = true\n";

#[test]
fn a_fixed_one_reads_only_the_shipped_file() {
    let places = Places::new();
    places.write(Layer::Shipped, "full.toml", FIXED);
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(
        Layer::Home,
        "full.toml",
        "[preset]\nname = \"Mine\"\n\n[tools]\nshell = false\n",
    );
    places.write(Layer::Home, "dev.toml", "[preset]\nname = \"My dev\"\n");
    let full = places.presets.find("full").unwrap();
    assert!(full.file.fixed);
    assert_eq!(full.layers, [Layer::Shipped], "家目录那一层不叠");
    assert_eq!(
        full.file.name.as_ref().and_then(|name| name.pick("en")),
        Some("Everything")
    );
    assert!(full.file.tools_off.is_empty());
    assert!(places.presets.fixed("full"));
    let dev = places.presets.find("dev").unwrap();
    assert!(!dev.file.fixed && !places.presets.fixed("dev"));
    assert_eq!(dev.layers, [Layer::Shipped, Layer::Home], "别的照旧叠");
    assert_eq!(places.presets.ignored(), ["full"]);
    assert!(!places.presets.fixed("nobody") && !places.presets.fixed("Bad"));
}

#[test]
fn only_the_shipped_layer_may_say_fixed() {
    let places = Places::new();
    places.write(Layer::Home, "mine.toml", "[preset]\nfixed = true\n");
    places.write(Layer::Package, "kit.toml", "[preset]\nfixed = true\n");
    for (id, layer) in [("mine", Layer::Home), ("kit", Layer::Package)] {
        match places.presets.find(id) {
            Err(PresetError::Invalid(at, _, problem)) => {
                assert_eq!((at, problem.code), (layer, Code::UnknownKey), "{id}");
                assert_eq!(problem.line, Some(2));
            }
            other => panic!("{id}：{other:?}"),
        }
        assert!(!places.presets.fixed(id));
    }
    assert!(places.presets.ignored().is_empty());
    assert_eq!(places.presets.check().len(), 2, "miyu check 也报");
    places.write(Layer::Shipped, "odd.toml", "[preset]\nfixed = \"yes\"\n");
    match places.presets.find("odd") {
        Err(PresetError::Invalid(_, _, problem)) => assert_eq!(problem.code, Code::NotBool),
        other => panic!("{other:?}"),
    }
}

/// 出厂的四份里只有全部功能、无功能定死了（2026-11-28 项目主人定）；定死不进指纹：以前造的快照照旧对得上。
#[test]
fn the_shipped_full_and_none_are_fixed() {
    let resources =
        ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let scratch = Scratch::new();
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let presets = Presets::new(&resources, &DataRoot::locate(&env).unwrap());
    for (id, fixed) in [
        ("full", true),
        ("none", true),
        ("dev", false),
        ("minimal", false),
    ] {
        let found = presets.find(id).unwrap();
        assert_eq!(
            (found.file.fixed, presets.fixed(id)),
            (fixed, fixed),
            "{id}"
        );
        let mut unfixed = found.file.clone();
        unfixed.fixed = false;
        assert_eq!(found.file.digest(), unfixed.digest(), "{id}");
    }
}

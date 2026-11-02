//! 哪种包带的人格删了是卸包（施工 PE-1）：只带这一个人格的出厂包才算；带了技能、别的人格、程序的，装上的包，你那一层或出厂
//! 资源里也有它的，都不算。

use std::path::Path;

use miyu_config::package::read;
use miyu_store::resources::ResourceRoot;

use super::*;
use crate::test_support::temp_root;

const BARE: &str = "[package]\nname = { en = \"Engineer\" }\nicon = \"code\"\n";

/// 在 `dir` 下放一个包：编号 `id`、清单 `manifest`，`files` 是包文件夹里另几份的相对位置。
fn package(dir: &Path, id: &str, layer: PackageLayer, manifest: &str, files: &[&str]) -> Package {
    let folder = dir.join(id);
    for relative in std::iter::once("package.toml").chain(files.iter().copied()) {
        let path = folder.join(relative);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(&path, manifest).expect("写得进");
    }
    Package {
        id: id.to_string(),
        layer,
        path: folder.join("package.toml"),
        read: Ok(read(manifest).expect("清单合写法")),
    }
}

const ONE: &str = "personas/engineer/persona.toml";

#[test]
fn only_a_package_with_just_this_persona_counts() {
    let (_root, dir) = temp_root("persona-only");
    let shipped = PackageLayer::Shipped;
    assert!(only_the_persona(
        &package(&dir, "bare", shipped, BARE, &[ONE]),
        "engineer"
    ));
    let logo = "[package]\nname = { en = \"E\" }\nlogo = \"logo.png\"\n";
    assert!(
        only_the_persona(
            &package(&dir, "logo", shipped, logo, &[ONE, "logo.png"]),
            "engineer"
        ),
        "图标不算别的内容"
    );
    let skill = package(&dir, "skill", shipped, BARE, &[ONE, "skills/x/SKILL.md"]);
    assert!(!only_the_persona(&skill, "engineer"), "带了技能");
    let two = package(
        &dir,
        "two",
        shipped,
        BARE,
        &[ONE, "personas/cat/persona.toml"],
    );
    assert!(!only_the_persona(&two, "engineer"), "带了两个人格");
    let program = "[package]\nprotocol = [1, 1]\nname = { en = \"E\" }\n\n[builtin]\n";
    let builtin = package(&dir, "builtin", shipped, program, &[ONE]);
    assert!(!only_the_persona(&builtin, "engineer"), "带了程序");
}

#[test]
fn the_sole_package_must_be_shipped_and_the_persona_nowhere_else() {
    let (root, dir) = temp_root("persona-sole");
    let resources = ResourceRoot::at(dir.join("resources"));
    let shipped = package(&dir, "eng", PackageLayer::Shipped, BARE, &[ONE]);
    let mine = package(
        &dir,
        "kitty",
        PackageLayer::Home,
        BARE,
        &["personas/cat/persona.toml"],
    );
    let folders = [shipped.files_dir(), mine.files_dir()];
    let personas = Personas::new(&resources, &root).with_packages(&folders);
    let packages = [shipped, mine];
    assert_eq!(
        sole_package(&packages, &personas, "engineer").as_deref(),
        Some("eng")
    );
    assert_eq!(sole_package(&packages, &personas, "cat"), None, "装上的包");
    assert_eq!(sole_package(&packages, &personas, "nobody"), None);
    std::fs::create_dir_all(root.path().join("root/personas/engineer")).expect("建得了");
    assert_eq!(
        sole_package(&packages, &personas, "engineer"),
        None,
        "你那一层也有的先删你那一层"
    );
}

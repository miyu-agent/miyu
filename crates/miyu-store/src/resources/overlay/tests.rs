//! 提示词的同名覆盖（施工 OV-1 中）：从上往下找第一份合规矩的；不合的照下一层、最后照出厂；`overlay_problems` 报不合的。

use std::fs;

use super::*;
use crate::test_support::Scratch;

/// 出厂的一份 `core/machine.txt`（带一个占位）和两层覆盖目录。
fn setup(temp: &Scratch) -> (ResourceRoot, PathBuf, PathBuf) {
    let shipped = temp.path().join("resources");
    fs::create_dir_all(shipped.join("core")).unwrap();
    fs::write(shipped.join("core/machine.txt"), "Shipped on {os}.\n").unwrap();
    let (mine, system) = (temp.path().join("mine"), temp.path().join("system"));
    for layer in [&mine, &system] {
        fs::create_dir_all(layer.join("core")).unwrap();
    }
    (ResourceRoot::at(shipped), mine, system)
}

#[test]
fn the_first_good_layer_wins_and_bad_ones_fall_through() {
    let temp = Scratch::new();
    let (root, mine, system) = setup(&temp);
    fs::write(system.join("core/machine.txt"), "System on {os}.\n").unwrap();
    let root = root.overlaid(vec![mine.clone(), system.clone()]);
    assert_eq!(
        root.read(&["core", "machine.txt"]).unwrap(),
        "System on {os}.\n",
        "自己的没写照系统的"
    );
    fs::write(mine.join("core/machine.txt"), "Mine on {os}.\n").unwrap();
    assert_eq!(
        root.read(&["core", "machine.txt"]).unwrap(),
        "Mine on {os}.\n",
        "自己的压着系统的"
    );
    fs::write(
        mine.join("core/machine.txt"),
        "Mine without the placeholder.\n",
    )
    .unwrap();
    assert_eq!(
        root.read(&["core", "machine.txt"]).unwrap(),
        "System on {os}.\n",
        "占位不对照下一层"
    );
    fs::write(
        system.join("core/machine.txt"),
        "这台电脑运行的系统是{os}。\n",
    )
    .unwrap();
    assert_eq!(
        root.read(&["core", "machine.txt"]).unwrap(),
        "Shipped on {os}.\n",
        "都不合照出厂"
    );
}

#[test]
fn problems_name_the_bad_files() {
    let temp = Scratch::new();
    let (root, mine, system) = setup(&temp);
    fs::write(mine.join("core/machine.txt"), "No placeholder.\n").unwrap();
    fs::write(mine.join("core/extra.txt"), "Nothing shipped.\n").unwrap();
    fs::write(system.join("core/view.json"), "{}").unwrap();
    fs::write(system.join("core/machine.txt"), "Good on {os}.\n").unwrap();
    let root = root.overlaid(vec![mine.clone(), system.clone()]);
    let problems = root.overlay_problems();
    let named: Vec<(String, &str)> = problems
        .iter()
        .map(|(path, why)| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                why.as_str(),
            )
        })
        .collect();
    assert_eq!(named.len(), 3, "{named:?}");
    assert!(
        named
            .iter()
            .any(|(file, why)| file == "machine.txt" && why.contains("placeholders"))
    );
    assert!(
        named
            .iter()
            .any(|(file, why)| file == "extra.txt" && why.contains("nothing shipped"))
    );
    assert!(
        named
            .iter()
            .any(|(file, why)| file == "view.json" && why.contains(".txt"))
    );
}

#[test]
fn json_and_unknown_texts_are_never_overlaid() {
    let temp = Scratch::new();
    let (root, mine, _) = setup(&temp);
    fs::write(root.path().join("core/view.json"), "{\"a\":1}").unwrap();
    fs::write(mine.join("core/view.json"), "{\"a\":2}").unwrap();
    let root = root.overlaid(vec![mine]);
    assert_eq!(root.read(&["core", "view.json"]).unwrap(), "{\"a\":1}");
}

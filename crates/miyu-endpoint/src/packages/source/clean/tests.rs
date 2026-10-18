//! 清缓存（施工 G-4 下）：装着那一版的（不分系统的、带系统的）留下，别的版本、别的包、下了一半的删掉。

use super::*;

#[test]
fn only_the_installed_versions_stay() {
    let dir = std::env::temp_dir().join(format!("miyu-clean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    for name in [
        "app-1.0.miyu",
        "app-0.9.miyu",
        "app-1.0.1.miyu",
        "bot-2.0-linux-x86_64.miyu",
        "gone-1.0.miyu",
        ".app-1.1.miyu.part",
    ] {
        std::fs::write(dir.join(name), b"1234").expect("写得进");
    }
    let keep = BTreeSet::from(["app-1.0".to_string(), "bot-2.0".to_string()]);
    assert_eq!(sweep(&dir, &keep), (4, 16));
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .expect("在")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["app-1.0.miyu", "bot-2.0-linux-x86_64.miyu"]);
    assert_eq!(
        sweep(&dir.join("none"), &keep),
        (0, 0),
        "没有缓存目录的什么都不做"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

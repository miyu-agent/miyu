use super::*;

#[test]
fn held_files_are_left_out_and_the_rest_keep_their_order() {
    let hidden: Vec<PathBuf> = ["a", "held", "b", "also-held", "c"]
        .iter()
        .map(PathBuf::from)
        .collect();
    let kept = deniable(&hidden, |path| path.to_string_lossy().contains("held"));
    assert_eq!(
        kept,
        ["a", "b", "c"]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    );
}

/// 真的独占打开一个文件（不让任何人共享）：它算被占着；放开以后、目录、不在的都不算。
#[cfg(windows)]
#[test]
fn a_file_opened_without_sharing_is_busy() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = std::env::temp_dir().join(format!("miyu-busy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let file = dir.join("hive.dat");
    std::fs::write(&file, b"x").expect("写得进");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&file)
        .expect("独占打开");
    assert!(busy(&file), "独占着的算");
    drop(held);
    assert!(!busy(&file), "放开了不算");
    assert!(!busy(&dir), "目录不算");
    assert!(!busy(&dir.join("nope")), "不在的不算");
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

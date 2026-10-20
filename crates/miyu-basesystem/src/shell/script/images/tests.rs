//! 交图的目录（施工 X-35）：建在缓存目录旁边、真实路径、Unix 上只给自己；两次建的不一样；丢掉就删。

use super::Folder;

#[test]
fn a_folder_is_private_real_and_removed_when_dropped() {
    let temp = std::env::temp_dir().join(format!("miyu-images-test-{}", std::process::id()));
    let cache = temp.join("tool");
    std::fs::create_dir_all(&cache).expect("建得了");
    let one = Folder::create(&cache).expect("建得成");
    let two = Folder::create(&cache).expect("建得成");
    assert_ne!(one.path(), two.path(), "每次一个新的");
    assert_eq!(
        one.path().canonicalize().expect("在"),
        one.path(),
        "交出去的是真实路径"
    );
    assert_eq!(
        one.path().parent(),
        temp.canonicalize().ok().as_deref(),
        "在缓存目录旁边"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(one.path())
            .expect("在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "只给自己");
    }
    let path = one.path().to_path_buf();
    std::fs::write(path.join("a.png"), b"x").expect("写得进");
    drop(one);
    assert!(!path.exists(), "丢掉就删，连里面的");
    drop(two);
    drop(std::fs::remove_dir_all(&temp));
}

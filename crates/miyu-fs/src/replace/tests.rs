//! 整体换成新的内容：新建、覆盖；只读的不写；盖不上去的，临时文件删掉，原来的没动；临时文件不比原来的宽（施工 SEC-3）。

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-replace-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // 换成真实的位置：交给 `replace` 的本来就是（macOS 上系统的临时目录在 `/var` 下，它是个链接）。
        Scratch(fs::canonicalize(dir).unwrap())
    }

    /// 目录里有些什么，照名字排。
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_created_and_then_replaced_whole() {
    let scratch = Scratch::new();
    let file = scratch.0.join("a.txt");
    replace(&file, b"one").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"one");
    replace(&file, b"two").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"two");
    assert_eq!(scratch.names(), ["a.txt"], "没留下临时文件");
}

#[test]
fn a_read_only_file_is_not_replaced() {
    let scratch = Scratch::new();
    let file = scratch.0.join("locked.txt");
    fs::write(&file, "keep").unwrap();
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions).unwrap();
    let error = replace(&file, b"new").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(&file).unwrap(), b"keep");
    assert_eq!(scratch.names(), ["locked.txt"]);
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "测试收尾：放开只读好删掉目录"
    )]
    permissions.set_readonly(false);
    fs::set_permissions(&file, permissions).unwrap();
}

#[test]
fn when_it_cannot_be_put_in_place_the_temporary_file_goes_away() {
    // 要盖的是一个不空的目录：临时文件写好了，改名盖不上去。
    let scratch = Scratch::new();
    let dir = scratch.0.join("dir");
    fs::create_dir_all(dir.join("inside")).unwrap();
    assert!(replace(&dir, b"x").is_err());
    assert_eq!(scratch.names(), ["dir"], "临时文件删掉了");
    assert!(dir.join("inside").is_dir(), "原来的没动");
}

/// 检查完以后，上级目录被换成了指向别处的链接（施工 5-10 下）：一个字节都不落到链接指的地方，也不留临时文件。
#[cfg(unix)]
#[test]
fn a_link_on_the_way_is_not_written_through() {
    let scratch = Scratch::new();
    let elsewhere = scratch.0.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, scratch.0.join("swapped")).unwrap();
    let through = fs::canonicalize(&scratch.0)
        .unwrap()
        .join("swapped")
        .join("new.txt");
    assert!(replace(&through, b"hi").is_err());
    assert!(
        fs::read_dir(&elsewhere).unwrap().next().is_none(),
        "链接指的地方什么都没写"
    );
    assert_eq!(scratch.names(), ["elsewhere", "swapped"], "也没留临时文件");
}

/// 写的时候盯着目录 `dir`：名字带 `marker` 的临时文件一出现就看它的权限位，交回看到的。写 `write` 一次没逮到的再写，
/// 最多 20 次（写得大，窗口有几十毫秒，一般头一次就逮到）。
#[cfg(unix)]
fn caught(dir: &std::path::Path, marker: &str, mut write: impl FnMut()) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    for _ in 0..20 {
        let done = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (dir, marker, done) = (dir.to_path_buf(), marker.to_string(), Arc::clone(&done));
            std::thread::spawn(move || {
                while !done.load(std::sync::atomic::Ordering::Relaxed) {
                    let Ok(entries) = fs::read_dir(&dir) else {
                        continue;
                    };
                    for entry in entries.flatten() {
                        if entry.file_name().to_string_lossy().contains(&marker)
                            && let Ok(metadata) = entry.metadata()
                        {
                            return Some(metadata.permissions().mode() & 0o777);
                        }
                    }
                }
                None
            })
        };
        write();
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(mode) = watcher.join().unwrap() {
            return mode;
        }
    }
    panic!("写了 20 次都没逮到临时文件");
}

/// 写的过程中临时文件不比原来的宽（施工 SEC-3，外部审计 L-2）：原来只有本人读得到的，临时文件建出来就是，不先照系统默认
/// 建成别人也读得到、写完才收回。
#[cfg(unix)]
#[test]
fn the_temporary_file_is_never_more_open_than_the_original() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let file = scratch.0.join("private.txt");
    fs::write(&file, "old").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let big = vec![b'x'; 64 << 20];
    let seen = caught(&scratch.0, ".miyu-tmp", || replace(&file, &big).unwrap());
    assert_eq!(seen & 0o077, 0, "临时文件建出来别人就读得到：{seen:o}");
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

//! 解开两种压缩包、查新版缺什么、对换（中途出错照原样退回）、收拾上次留下的。

use super::*;

/// 测试的临时目录，用完删掉。
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-cli-upgrade-{name}-{}", std::process::id()));
        if dir.exists() {
            miyu_store::testkit::remove_tree(&dir);
        }
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }

    pub(crate) fn put(&self, file: &str, text: &str) -> PathBuf {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        fs::write(&path, text).expect("写得进");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        miyu_store::testkit::remove_tree(&self.0);
    }
}

/// 一个新版的目录 `<root>/miyu-<版本>-t/`：五个程序（写着 `new`）、一份资源。
pub(crate) fn new_version(scratch: &Scratch, top: &str) -> PathBuf {
    for program in PROGRAMS {
        scratch.put(&format!("{top}/{program}"), "new");
    }
    scratch.put(&format!("{top}/resources/core/a.md"), "new resource");
    scratch.0.join(top)
}

/// 照 `dir`（相对 `scratch`）打一个 tar.gz：里面一层 `name/`。
pub(crate) fn tar_gz(dir: &Path, name: &str) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    tar.append_dir_all(name, dir).expect("打得进");
    tar.into_inner().expect("收得了").finish().expect("压得完")
}

#[test]
fn both_kinds_unpack_into_one_folder() {
    let scratch = Scratch::new("unpack");
    let top = new_version(&scratch, "src/miyu-1.0.0-t");
    let tgz = scratch.0.join("a.tar.gz");
    fs::write(&tgz, tar_gz(&top, "miyu-1.0.0-t")).unwrap();
    unpack(&tgz, &scratch.0.join("out-tar")).unwrap();
    assert_eq!(
        fs::read_to_string(scratch.0.join("out-tar/miyu-1.0.0-t/resources/core/a.md")).unwrap(),
        "new resource"
    );

    let zipped = scratch.0.join("a.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&zipped).unwrap());
    zip.start_file(
        "miyu-1.0.0-t/miyu",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    io::Write::write_all(&mut zip, b"new").unwrap();
    zip.finish().unwrap();
    unpack(&zipped, &scratch.0.join("out-zip")).unwrap();
    assert_eq!(
        fs::read_to_string(scratch.0.join("out-zip/miyu-1.0.0-t/miyu")).unwrap(),
        "new"
    );

    let broken = scratch.put("b.tar.gz", "not gzip");
    assert!(unpack(&broken, &scratch.0.join("out-bad")).is_err());
}

#[test]
fn what_the_new_version_lacks_is_listed() {
    let scratch = Scratch::new("missing");
    let top = new_version(&scratch, "miyu-1.0.0-t");
    assert_eq!(missing(&top, ""), Vec::<String>::new());
    fs::remove_file(top.join("miyu-web")).unwrap();
    miyu_store::testkit::remove_tree(&top.join("resources"));
    assert_eq!(
        missing(&top, ""),
        vec!["miyu-web".to_string(), "resources/".to_string()]
    );
    assert_eq!(missing(&top, ".exe").len(), 6, "Windows 上找带 .exe 的");
}

/// 装着的旧版：两个程序、一份资源、标记、上次没删的暂存。
fn installed(scratch: &Scratch) -> PathBuf {
    scratch.put("install/miyu", "old");
    scratch.put("install/miyu-tui", "old");
    scratch.put("install/resources/core/a.md", "old resource");
    scratch.put("install/install.toml", "channel = \"script\"\n");
    scratch.0.join("install")
}

#[test]
fn swapping_puts_the_new_files_in_keeps_the_marker_and_removes_the_old() {
    let scratch = Scratch::new("swap");
    let dir = installed(&scratch);
    let top = new_version(&scratch, "install/.miyu-upgrade/unpacked/miyu-1.0.0-t");
    let old = dir.join(".miyu-old-0.9.0");
    let swapped = swap(&dir, &top, &old, "", &mut |from, to| fs::rename(from, to)).unwrap();
    assert_eq!(
        swapped,
        Swapped {
            left: None,
            kept: Vec::new()
        }
    );
    assert_eq!(fs::read_to_string(dir.join("miyu")).unwrap(), "new");
    assert_eq!(
        fs::read_to_string(dir.join("resources/core/a.md")).unwrap(),
        "new resource"
    );
    assert_eq!(fs::read_to_string(dir.join("miyu-web")).unwrap(), "new");
    assert_eq!(
        fs::read_to_string(dir.join("install.toml")).unwrap(),
        "channel = \"script\"\n"
    );
    assert!(!old.exists(), "旧版删了");
    assert!(
        dir.join(".miyu-upgrade").is_dir(),
        "暂存不动（由调用的一方删）"
    );
}

#[test]
fn a_failure_halfway_puts_everything_back() {
    for fail_at in [1, 3, 5] {
        let scratch = Scratch::new(&format!("rollback-{fail_at}"));
        let dir = installed(&scratch);
        let top = new_version(&scratch, "install/.miyu-upgrade/unpacked/miyu-1.0.0-t");
        let old = dir.join(".miyu-old-0.9.0");
        let mut count = 0;
        let mut rename = |from: &Path, to: &Path| {
            count += 1;
            match count == fail_at {
                true => Err(io::Error::other("disk full")),
                false => fs::rename(from, to),
            }
        };
        let (step, why) = swap(&dir, &top, &old, "", &mut rename).unwrap_err();
        assert!(step.starts_with("move-"), "{step}");
        assert_eq!(why, "disk full");
        assert_eq!(
            fs::read_to_string(dir.join("miyu")).unwrap(),
            "old",
            "第 {fail_at} 步：旧的放回去了"
        );
        assert_eq!(
            fs::read_to_string(dir.join("resources/core/a.md")).unwrap(),
            "old resource"
        );
        assert!(!dir.join("miyu-web").exists(), "新的退回暂存");
        assert_eq!(fs::read_to_string(top.join("miyu-web")).unwrap(), "new");
        assert_eq!(
            fs::read_dir(&top).unwrap().count(),
            6,
            "挪进来的也退回暂存：新版六样都在"
        );
        assert!(!old.exists(), "挪回去以后空的旧版目录删了");
    }
}

#[test]
fn tidying_removes_leftovers_only() {
    let scratch = Scratch::new("tidy");
    let dir = installed(&scratch);
    scratch.put("install/.miyu-old-0.8.0/miyu", "older");
    scratch.put("install/.miyu-upgrade/x", "half");
    tidy(&dir);
    assert!(!dir.join(".miyu-old-0.8.0").exists());
    assert!(!dir.join(".miyu-upgrade").exists());
    assert!(
        dir.join("miyu").is_file()
            && dir.join("install.toml").is_file()
            && dir.join("resources").is_dir()
    );
    tidy(&scratch.0.join("nowhere"));
}

/// 换哪几样照第一层分（施工 RL-4补3）：新压缩包有的、固定名单上的（新版拿掉了的程序也清掉）整样换；别的原样留着、交回名字；
/// `resources/` 整个换，放在里面的没了。
#[test]
fn only_miyus_own_first_level_entries_are_swapped() {
    let scratch = Scratch::new("owned");
    let dir = installed(&scratch);
    scratch.put("install/notes.txt", "人自己放的");
    scratch.put("install/my-tools/run.sh", "人自己放的目录");
    scratch.put("install/miyu-extract", "旧的");
    scratch.put("install/README.txt", "旧的说明");
    scratch.put("install/resources/old.txt", "旧资源");
    let top = new_version(&scratch, "install/.miyu-upgrade/unpacked/miyu-1.0.0-t");
    // 新版拿掉了 miyu-extract、没带 README.txt：名单上有，旧的照样清掉。
    fs::remove_file(top.join("miyu-extract")).unwrap();
    let old = dir.join(".miyu-old-0.9.0");
    let swapped = swap(&dir, &top, &old, "", &mut |from, to| fs::rename(from, to)).unwrap();
    assert_eq!(
        swapped.kept,
        vec!["my-tools".to_string(), "notes.txt".to_string()]
    );
    assert_eq!(
        fs::read_to_string(dir.join("notes.txt")).unwrap(),
        "人自己放的"
    );
    assert!(dir.join("my-tools/run.sh").is_file());
    assert!(!dir.join("miyu-extract").exists(), "名单上的旧程序清掉");
    assert!(!dir.join("README.txt").exists(), "名单上的旧说明清掉");
    assert!(!dir.join("resources/old.txt").exists(), "resources/ 整个换");
    assert_eq!(fs::read_to_string(dir.join("miyu")).unwrap(), "new");
    assert!(dir.join("install.toml").is_file());
    assert!(!old.exists());
}

#[test]
fn on_windows_the_owned_programs_carry_exe() {
    let scratch = Scratch::new("owned-exe");
    let dir = installed(&scratch);
    scratch.put("install/miyu-extract.exe", "旧的");
    scratch.put("install/miyu-extract", "不带后缀的不是程序");
    let top = new_version(&scratch, "install/.miyu-upgrade/unpacked/miyu-1.0.0-t");
    fs::remove_file(top.join("miyu-extract")).unwrap();
    let old = dir.join(".miyu-old-0.9.0");
    let swapped = swap(&dir, &top, &old, ".exe", &mut |from, to| {
        fs::rename(from, to)
    })
    .unwrap();
    assert!(!dir.join("miyu-extract.exe").exists());
    assert_eq!(
        swapped.kept,
        vec!["miyu-extract".to_string()],
        "Windows 上名单照 .exe 认"
    );
}

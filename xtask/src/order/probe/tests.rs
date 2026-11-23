//! 探针那一轮里不编、不跑的几样：碰过的页的文件名、发行包目录里不许有探针。

use super::*;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("xtask-order-probe-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("清得掉上次留下的");
        }
        fs::create_dir_all(dir.join("resources").join("packages")).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

#[test]
fn the_touched_pages_sit_next_to_the_list() {
    assert_eq!(
        touched(Path::new("/w/target/order/macos/probe-miyu.txt")),
        Path::new("/w/target/order/macos/probe-miyu.txt.pages")
    );
}

#[test]
fn a_clean_directory_passes_and_a_probe_in_it_is_found() {
    let scratch = Scratch::new("clean");
    let library = Path::new("/w/target/order/pagetrace.dylib");
    fs::write(scratch.0.join("miyu"), b"\xcf\xfa\xed\xfe program").unwrap();
    fs::write(scratch.0.join("resources/packages/a.toml"), "x = 1\n").unwrap();
    clean(&scratch.0, library).unwrap();

    // 照文件名认：拷进来了。
    fs::write(scratch.0.join("resources/pagetrace.dylib"), b"anything").unwrap();
    let error = clean(&scratch.0, library).unwrap_err();
    // 找到的那一份在 resources 下面（探针自己的路径里没有这个词）。
    assert!(error.contains("resources"), "{error}");
    fs::remove_file(scratch.0.join("resources/pagetrace.dylib")).unwrap();

    // 照安装名认：改了名的动态库里还写着它编出来时的路径。
    let mut renamed = b"\xcf\xfa\xed\xfe LC_ID_DYLIB ".to_vec();
    renamed.extend_from_slice(library.as_os_str().as_encoded_bytes());
    fs::write(scratch.0.join("resources/packages/libx.dylib"), renamed).unwrap();
    let error = clean(&scratch.0, library).unwrap_err();
    assert!(error.contains("libx.dylib"), "{error}");
}

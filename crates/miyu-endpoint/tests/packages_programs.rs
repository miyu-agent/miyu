//! 包自己带的程序（施工 F-8 上再补，`docs/blueprint/packages.md`「转交」第 5 条、`extensions.md`「怎么走」第 1 条）：程序放在
//! 包文件夹的 `bin/` 里，装的时候随包拷进来，核心照它拉起，不用放到 `miyu` 旁边。刚拷进来的程序还被别的进程开着写的，等它
//! 放开再拉起，不当起不来。

use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 测试用的扩展编出来在哪。
const BUILT: &str = env!("CARGO_BIN_EXE_miyu-test-extension");

/// 一个只在这个测试里用的程序名：测试程序旁边没有它，找得到就只能是包里的 `bin/`。
fn own_name(tag: &str) -> String {
    format!("miyu-own-{tag}-{}", std::process::id())
}

/// 把测试用的扩展放进 `bin` 叫 `name`。Unix 上放链接：装的时候照它指的拷，不在这里开着写的句柄；Windows 上拷。
fn bring(bin: &Path, name: &str) {
    std::fs::create_dir_all(bin).expect("建得了");
    let target = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    #[cfg(unix)]
    std::os::unix::fs::symlink(BUILT, &target).expect("链得上");
    #[cfg(not(unix))]
    std::fs::copy(BUILT, &target).expect("拷得了");
}

#[tokio::test]
async fn a_package_brings_its_own_program() {
    let home = Home::new();
    let id = "xown";
    let program = own_name("install");
    let (path, step) = record(&home, id);
    install(
        &home,
        id,
        &program,
        "always",
        &steps(&[&step, "hello", "serve"]),
    );
    let written = home.root.path().join(format!("home/alice/packages/{id}"));
    let source = home.work.join(id);
    std::fs::rename(&written, &source).expect("挪得动");
    bring(&source.join("bin"), &program);

    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], id, "{reply}");
    until_state(&mut client, id, |entry| entry["state"] == "running").await;
    until("扩展记下拉起它的参数", || path.is_file()).await;

    let listed = client.call("l", "package.list", json!({})).await;
    let entry = listed["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .expect("列得出");
    assert_eq!(entry["status"], "running", "{entry}");
    // 程序在包里的也算装了（`package-pages.md`「程序不在就当没装」）：它的功能在预设里是装了的。
    let preset = client
        .call("g", "preset.get", json!({"preset": "full"}))
        .await;
    let feature = preset["result"]["features"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["id"] == id)
        .cloned()
        .expect("列得出");
    assert_eq!(feature["installed"], true, "{feature}");
    let files = client
        .call("f", "package.files", json!({"package": id}))
        .await;
    let file = format!("bin/{program}{}", std::env::consts::EXE_SUFFIX);
    assert!(
        files.to_string().contains(&file),
        "装上的文件里有它带的程序：{files}"
    );
    core.stop_extensions().await;
}

/// 程序刚换上、还被别的进程开着写（Linux 回 `ETXTBSY`）：等它放开再拉起，不记成起不来。macOS、Windows 不这样报，只在 Linux 上测。
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_busy_program_starts_once_it_is_let_go() {
    let home = Home::new();
    let id = "xbusy";
    let program = own_name("busy");
    install(&home, id, &program, "always", &steps(&["hello", "serve"]));
    let bin = home
        .root
        .path()
        .join(format!("home/alice/packages/{id}/bin"));
    std::fs::create_dir_all(&bin).expect("建得了");
    let target = bin.join(&program);
    std::fs::copy(BUILT, &target).expect("拷得了");
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .expect("开得了");

    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let now = until_state(&mut client, id, |_| true).await;
    assert_ne!(now["state"], "stopped", "开着写的时候不算起不来：{now}");
    drop(writer);
    until_state(&mut client, id, |entry| entry["state"] == "running").await;
    core.stop_extensions().await;
}

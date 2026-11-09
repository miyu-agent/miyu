//! `miyu pkg page <编号>`（施工 BP-2）：目录里的后台页照核心 `package.page` 交回的宿主页地址印出来；后台页是程序自己起的服务、程序
//! 没在跑的说一句，退出码 1；没装的照核心的原话说。测试都带 `--print`，不开浏览器。

use crate::pkg_source::pkg_speaking;
use crate::support::Home;
use miyu_cli::language::Language;
use miyu_session::testkit::Script;

/// 经 `miyu pkg install` 装一个扩展 `id`（程序不在），后台页那一节写 `page`：包文件夹放在现在的目录下。
async fn installed(home: &Home, id: &str, page: &str) {
    let dir = home.outside().join(id);
    std::fs::create_dir_all(dir.join("page")).expect("建得了");
    std::fs::write(
        dir.join("package.toml"),
        format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"Relay\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-no-such-program\"\nabout = {{ en = \"R\" }}\n\n[process]\nstart = \"manual\"\n\n[page]\n{page}\n"),
    )
    .expect("写得进");
    std::fs::write(dir.join("page/index.html"), "<p>relay</p>").expect("写得进");
    let folder = format!("./{id}");
    let (code, tape) =
        pkg_speaking(home, &["install", &folder, "--yes"], b"", Language::Chinese).await;
    assert_eq!(code, 0, "{}", tape.text(|err| err));
}

#[tokio::test]
async fn a_page_in_the_package_prints_the_host_address() {
    let home = Home::new(std::sync::Arc::new(Script::new([])));
    installed(&home, "relay", "dir = \"page\"").await;
    let (code, tape) =
        pkg_speaking(&home, &["page", "relay", "--print"], b"", Language::Chinese).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let url = out.trim_end();
    assert!(
        url.starts_with("http://127.0.0.1:") && url.contains("/h/") && url.ends_with('/'),
        "{url}"
    );
}

#[tokio::test]
async fn a_served_page_whose_program_is_not_running_says_so() {
    let home = Home::new(std::sync::Arc::new(Script::new([])));
    installed(&home, "site", "server = true").await;
    let (code, tape) =
        pkg_speaking(&home, &["page", "site", "--print"], b"", Language::Chinese).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("site 没有在运行"), "{err}");
}

#[tokio::test]
async fn an_unknown_package_is_refused() {
    let home = Home::new(std::sync::Arc::new(Script::new([])));
    let (code, tape) = pkg_speaking(
        &home,
        &["page", "nothing", "--print"],
        b"",
        Language::Chinese,
    )
    .await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(!err.is_empty());
}

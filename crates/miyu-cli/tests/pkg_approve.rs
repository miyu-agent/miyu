//! 装的时候就批能力（施工 PK-3 下，设计 `61-装包说清来源和能力.md` 第三节、第四节第 2 条、定了的 6）：在进程里起的核心，从包
//! 文件夹装一个要能力的扩展。确认屏照核心给的字逐字对（中、英）：来源写本机文件、能力一项一行带说明（照能力表的先后）、「注意」接在问的前面、问
//! 「并批准」；答「是」、写 `--yes` 的都带上 `approve`，开关文件里记下批过的，输出列出批了哪些；答「否」的什么都不装、不批。

use std::sync::Arc;

use serde_json::Value;

use crate::pkg_source::pkg_speaking;
use crate::support::Home;
use miyu_cli::language::Language;
use miyu_session::testkit::Script;

/// 程序一定不在的那个名字：只装、不拉起（装好以后也不问「现在启用？」）。
const NOWHERE: &str = "miyu-no-such-program-anywhere";

/// 现在的目录（`home.outside()`）下放一个扩展包 `kit`：要联网、提供工具，`start = "manual"`。交回清单的字节数。
fn folder(home: &Home) -> usize {
    let dir = home.outside().join("kit");
    std::fs::create_dir_all(&dir).expect("建得了");
    let manifest = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Kit\" }}\n\n[command]\nname = \"kit\"\nprogram = \"{NOWHERE}\"\n\
         about = {{ en = \"K\" }}\n\n[process]\nargs = [\"serve\"]\nstart = \"manual\"\ncapabilities = [\"network\", \"tools\"]\n"
    );
    std::fs::write(dir.join("package.toml"), &manifest).expect("写得进");
    manifest.len()
}

/// 开关文件里 `kit` 批过的；没记的是 `null`。
fn approved(home: &Home) -> Value {
    approved_of(home, "kit")
}

/// 开关文件里 `id` 批过的；没记的是 `null`。
fn approved_of(home: &Home, id: &str) -> Value {
    let text = std::fs::read_to_string(home.root.extensions_file()).unwrap_or_default();
    let switches: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    switches["approved"][id].clone()
}

fn home() -> Home {
    Home::new(Arc::new(Script::new([])))
}

#[tokio::test]
async fn the_screen_says_where_and_what_and_yes_approves_it() {
    let home = home();
    let bytes = folder(&home);
    let (code, tape) = pkg_speaking(&home, &["install", "./kit"], b"y\n", Language::Chinese).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    let stages = "(1/1) 正在核对完整性\n(1/1) 正在安装 kit\n";
    assert_eq!((code, err.as_str()), (0, stages), "{out}");
    assert_eq!(
        out,
        format!(
            "将安装 kit\n包含：扩展程序、命令 miyu kit\n需要：提供工具：给 AI 加能调用的工具\n      联网：访问网络、开端口\n\
             来源：本机文件\n大小：{bytes} B，1 个文件\n\n\
             注意：这个包没经过官方审核。它带的程序会以你的身份在这台机器上跑，不在沙盒里。\n\
             安装 'kit' 并批准它要的能力？[Y/n] 已安装 kit\n已批准 kit 的能力：提供工具、联网\n"
        )
    );
    assert_eq!(approved(&home), serde_json::json!(["network", "tools"]));
}

#[tokio::test]
async fn the_screen_reads_the_same_in_english_and_no_approves_nothing() {
    let home = home();
    let bytes = folder(&home);
    let (code, tape) = pkg_speaking(&home, &["install", "./kit"], b"n\n", Language::English).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, err.as_str()), (1, "cancelled\n"), "{out}");
    assert_eq!(
        out,
        format!(
            "Will install kit\nIncludes: extension, command miyu kit\nNeeds: Provide tools: Add tools the AI can call\n       \
             Network: Access the network and open ports\nFrom: local file\nSize: {bytes} B, 1 file\n\n\
             Note: This package has not been officially reviewed. The programs it carries run as you on this machine, \
             outside any sandbox.\nInstall 'kit' and approve the capabilities it needs? [Y/n] "
        )
    );
    assert!(!home.root.packages().join("kit").exists(), "答「否」的不装");
    assert_eq!(approved(&home), Value::Null);
}

#[tokio::test]
async fn yes_skips_the_question_but_still_approves_and_says_so() {
    let home = home();
    folder(&home);
    let (code, tape) = pkg_speaking(
        &home,
        &["install", "./kit", "--noconfirm"],
        b"",
        Language::English,
    )
    .await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    let stages = "(1/1) checking package integrity\n(1/1) installing kit\n";
    assert_eq!((code, err.as_str()), (0, stages), "{out}");
    assert_eq!(
        out,
        "installed kit\napproved capabilities for kit: Provide tools, Network\n"
    );
    assert_eq!(approved(&home), serde_json::json!(["network", "tools"]));
}

/// 打一个扩展包 `id` 的 `version` 版：要 `capabilities` 这几样。交回文件名、字节。
fn extension_file(
    dir: &std::path::Path,
    (id, version): (&str, &str),
    capabilities: &[&str],
) -> (String, Vec<u8>) {
    let folder = dir.join("src").join(version).join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let names: Vec<String> = capabilities.iter().map(|one| format!("{one:?}")).collect();
    std::fs::write(
        folder.join("package.toml"),
        format!(
            "[package]\nversion = \"{version}\"\nprotocol = [1, 1]\nname = {{ en = \"App\" }}\n\n[command]\nname = \"{id}\"\n\
             program = \"{NOWHERE}\"\nabout = {{ en = \"A\" }}\n\n[process]\nstart = \"manual\"\ncapabilities = [{}]\n",
            names.join(", ")
        ),
    )
    .expect("写得进");
    let name = miyu_source::archive::file_name(id, version, None);
    let out = dir.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    miyu_source::archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

/// 升级（设计 61 定了的 5、6）：2.0 新要联网，确认屏在「将升级」下面列出来、问「并批准新要的能力」；`--yes` 不问照样批，输出
/// 列出批了哪些。
#[tokio::test]
async fn an_upgrade_lists_what_is_newly_needed_and_yes_approves_it() {
    use crate::pkg_source::{entry, home_with_mirror, index_replies, keys};
    use miyu_http::testkit::{Piece, Reply, Server};

    let (secret, trusted) = keys();
    let scratch = std::env::temp_dir().join(format!("miyu-cli-approve-{}", std::process::id()));
    let one = extension_file(&scratch, ("app", "1.0"), &[]);
    let two = extension_file(&scratch, ("app", "2.0"), &["network"]);
    let mut first = entry("app", &[], &one);
    first["kind"] = serde_json::json!("process");
    let mut second = entry("app", &[], &two);
    second["version"] = serde_json::json!("2.0");
    second["kind"] = serde_json::json!("process");
    second["capabilities"] = serde_json::json!(["network"]);
    let mut replies = index_replies("2026-10-19T03:00:00Z", vec![first], &secret);
    replies.push(Reply::stream(vec![Piece::Bytes(one.1.clone())]));
    replies.extend(index_replies("2026-10-20T03:00:00Z", vec![second], &secret));
    replies.push(Reply::stream(vec![Piece::Bytes(two.1.clone())]));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let home = home_with_mirror("https://unreachable.invalid/", &mirror, trusted);
    let (code, _) = pkg_speaking(&home, &["-S", "app", "--yes"], b"", Language::Chinese).await;
    assert_eq!(code, 0);
    let (code, _) = pkg_speaking(&home, &["-Sy"], b"", Language::Chinese).await;
    assert_eq!(code, 0);

    let (code, tape) = pkg_speaking(&home, &["upgrade"], b"n\n", Language::Chinese).await;
    let out = tape.text(|err| !err);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.starts_with("将升级 app 1.0 → 2.0\n  新要：联网：访问网络、开端口\n下载："),
        "{out}"
    );
    assert!(
        out.ends_with("\n\n升级 'app' 并批准新要的能力？[Y/n] "),
        "官方的没有「注意」：{out}"
    );
    assert_eq!(approved_of(&home, "app"), Value::Null, "答「否」的不批");

    let (code, tape) = pkg_speaking(&home, &["-Su", "--yes"], b"", Language::Chinese).await;
    let out = tape.text(|err| !err);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, "已升级 app 1.0 → 2.0\n已批准 app 的能力：联网\n");
    assert_eq!(approved_of(&home, "app"), serde_json::json!(["network"]));
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 推荐的（设计 61 第三节「推荐的」）：第二句之前另印一段推荐的包要的能力，答「是」另装另批。
#[tokio::test]
async fn a_recommended_package_is_shown_apart_and_approved_when_taken() {
    use crate::pkg_source::{entry, home_with_mirror, index_replies, keys};
    use miyu_http::testkit::{Piece, Reply, Server};

    let (secret, trusted) = keys();
    let scratch = std::env::temp_dir().join(format!("miyu-cli-recommend-{}", std::process::id()));
    let app = extension_file(&scratch, ("app", "1.0"), &[]);
    let rec = extension_file(&scratch, ("rec", "1.0"), &["network"]);
    let mut wanted = entry("app", &[], &app);
    wanted["recommends"] = serde_json::json!(["rec"]);
    let mut offered = entry("rec", &[], &rec);
    offered["kind"] = serde_json::json!("process");
    offered["capabilities"] = serde_json::json!(["network"]);
    let mut replies = index_replies("2026-10-19T03:00:00Z", vec![wanted, offered], &secret);
    replies.push(Reply::stream(vec![Piece::Bytes(app.1.clone())]));
    replies.push(Reply::stream(vec![Piece::Bytes(rec.1.clone())]));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let home = home_with_mirror("https://unreachable.invalid/", &mirror, trusted);
    let (code, tape) = pkg_speaking(&home, &["-S", "app"], b"y\ny\n", Language::Chinese).await;
    let out = tape.text(|err| !err);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "\n安装 'app'？[Y/n] \n推荐的包要的能力  : rec  联网：访问网络、开端口\n\
             同时安装推荐的 1 个软件包并批准它要的能力？[Y/n] "
        ),
        "{out}"
    );
    assert!(
        out.ends_with("已安装 rec 1.0\n已批准 rec 的能力：联网\n"),
        "{out}"
    );
    assert_eq!(approved_of(&home, "rec"), serde_json::json!(["network"]));
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
}

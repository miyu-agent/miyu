//! 数据根里的密钥哪一级都碰不到（施工 PV-RM，2026-11-16 项目主人定：私密目录去掉，数据根一个字节都不松）：只读的沙盒规格里
//! 藏的只有数据根，家目录里别的程序的钥匙不藏；文件工具完全放开、只读都读不到 `etc/private/secrets.toml`，只读时读家目录里
//! 的钥匙不问人。只读时命令读不到密钥见 `sandbox.rs` 的 `a_read_only_command_reads_a_key_folder_but_not_the_secrets`。
//! 场地的假家目录是 `home/`。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 真实的位置。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("在")
}

/// 在 `cwd` 里造一个只读的会话，沙盒能用（假的助手，假工具不起它）：执行命令的假工具 `run` 跑一次，交回它拿到的沙盒。
async fn attached(home: &Home, cwd: &Path) -> Arc<Sandboxed> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: true,
        },
        attended: false,
        cwd: cwd.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let calls = run.calls();
    assert_eq!(calls.len(), 1, "跑了一次");
    calls[0].sandbox.clone().expect("进了沙盒")
}

#[tokio::test]
async fn the_read_only_spec_hides_only_the_data_root() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    for dir in [".claude", ".ssh", ".config/gh"] {
        std::fs::create_dir_all(home.home.join(dir)).expect("建得了目录");
    }
    let sandboxed = attached(&home, &work).await;
    assert_eq!(
        sandboxed.spec.hidden,
        [real(home.root.path())],
        "别的程序的钥匙、登录不藏"
    );
}

/// 在 `work/` 里造会话、用出厂的 `read` 读 `path` 一次：完全放开、只读 `read_only`，没人能确认（要问人的就拒了）。交回那一条结果。
async fn read_once(home: &Home, path: &Path, read_only: bool) -> ToolResult {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "file_path": path.to_string_lossy() }).to_string();
    let script = Script::new([Play::calls(&[("read", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果")
}

/// 结果里给她看的字。
fn text(result: &ToolResult) -> String {
    result
        .blocks
        .iter()
        .filter_map(|block| match block {
            miyu_kernel::block::Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn file_tools_cannot_read_the_secrets_at_either_level() {
    let home = Home::outside_temp();
    let secrets = home.root.secrets_file();
    std::fs::create_dir_all(secrets.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&secrets, "secret-in-data-root\n").expect("写得进");
    for read_only in [false, true] {
        let result = read_once(&home, &secrets, read_only).await;
        assert_eq!(result.status, ToolStatus::Denied, "只读 {read_only}");
        assert!(
            !text(&result).contains("secret-in-data-root"),
            "只读 {read_only}：读到了密钥：{}",
            text(&result)
        );
    }
}

/// 只读时读家目录里别的程序的钥匙不问人（施工 PV-RM）：没人能确认也照读。
#[tokio::test]
async fn a_read_only_file_tool_reads_a_key_folder_without_asking() {
    let home = Home::outside_temp();
    let key = home.home.join(".ssh/config");
    std::fs::create_dir_all(key.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&key, "Host *\n").expect("写得进");
    let result = read_once(&home, &key, true).await;
    assert_eq!(result.status, ToolStatus::Ok, "{result:?}");
    assert!(text(&result).contains("Host *"), "{}", text(&result));
}

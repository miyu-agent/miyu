//! 数据根两种模式都读得到，完全放开下写得进、只读写不进（施工 RO-2、RO-3，2026-11-16、11-17 项目主人定）：只读的
//! 沙盒规格什么都不藏；出厂的 `read` 完全放开、只读都读得到 `etc/private/secrets.toml`；`write`、`edit` 完全放开下写得进
//! 数据根（施工 RO-3），只读时内核先拦下；只读时读家目录里的钥匙不问人。只读时命令读得到密钥、写不进数据根见 `sandbox.rs` 的
//! `a_read_only_command_reads_the_secrets_but_cannot_write_the_data_root`。场地的假家目录是 `home/`。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

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
async fn the_read_only_spec_hides_nothing() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    for dir in [".claude", ".ssh", ".config/gh"] {
        std::fs::create_dir_all(home.home.join(dir)).expect("建得了目录");
    }
    let sandboxed = attached(&home, &work).await;
    assert!(
        sandboxed.spec.hidden.is_empty(),
        "数据根、别的程序的钥匙都不藏：{:?}",
        sandboxed.spec.hidden
    );
    assert!(sandboxed.spec.reveal.is_empty(), "没藏的，不用另放开");
    assert!(sandboxed.spec.write.is_empty(), "哪儿都不能写");
}

/// 在 `work/` 里造会话、用出厂的工具调一次 `tool`（参数 `args`）：完全放开、只读 `read_only`，没人能确认（要问人的就拒了）。
/// 交回那一条结果。
async fn once(home: &Home, (tool, args): (&str, &str), read_only: bool) -> ToolResult {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let script = Script::new([Play::calls(&[(tool, args)]), Play::Says("好。")]);
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

/// 用出厂的 `read` 读 `path` 一次（[`once`]）。
async fn read_once(home: &Home, path: &Path, read_only: bool) -> ToolResult {
    let args = serde_json::json!({ "file_path": path.to_string_lossy() }).to_string();
    once(home, ("read", &args), read_only).await
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

/// 造好数据根里的 `etc/private/secrets.toml`，交回它在哪。
fn secrets(home: &Home) -> PathBuf {
    let secrets = home.root.secrets_file();
    std::fs::create_dir_all(secrets.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&secrets, "secret-in-data-root\n").expect("写得进");
    secrets
}

#[tokio::test]
async fn file_tools_read_the_secrets_at_either_level() {
    let home = Home::outside_temp();
    let secrets = secrets(&home);
    for read_only in [false, true] {
        let result = read_once(&home, &secrets, read_only).await;
        assert_eq!(
            result.status,
            ToolStatus::Ok,
            "只读 {read_only}：{result:?}"
        );
        assert!(
            text(&result).contains("secret-in-data-root"),
            "只读 {read_only}：{}",
            text(&result)
        );
    }
}

#[tokio::test]
async fn file_tools_write_the_data_root_only_at_full_access() {
    let path = |at: &Path| at.to_string_lossy().into_owned();
    for read_only in [true, false] {
        let home = Home::outside_temp();
        let secrets = secrets(&home);
        let fresh = home.root.path().join("etc/new.txt");
        let calls = [
            (
                "write",
                serde_json::json!({ "file_path": path(&fresh), "content": "x" }),
            ),
            (
                "edit",
                serde_json::json!({
                    "file_path": path(&secrets),
                    "old_string": "secret",
                    "new_string": "changed",
                }),
            ),
        ];
        for (tool, args) in &calls {
            let result = once(&home, (tool, &args.to_string()), read_only).await;
            // 只读时写入类的调用内核先拦下；完全放开同别的目录一样写得进（施工 RO-3）。
            let expected = if read_only {
                ToolStatus::Denied
            } else {
                ToolStatus::Ok
            };
            assert_eq!(
                result.status, expected,
                "{tool} 只读 {read_only}：{result:?}"
            );
            if read_only {
                assert!(
                    text(&result).ends_with("Not run: the session is read-only.\n"),
                    "{tool}：{}",
                    text(&result)
                );
            }
        }
        let (now, wrote) = match read_only {
            true => ("secret-in-data-root\n", false),
            false => ("changed-in-data-root\n", true),
        };
        assert_eq!(std::fs::read_to_string(&secrets).expect("还在"), now);
        assert_eq!(fresh.exists(), wrote, "只读 {read_only}");
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

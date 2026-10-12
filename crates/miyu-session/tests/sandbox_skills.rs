//! 沙盒里的命令读得到开着的技能文件夹（施工 X-10 中，`docs/blueprint/skills.md`「技能」第 3 条）：数据根照旧藏着，技能的
//! 文件夹在规格里单独放开只读（本机的会话写进 `reveal`，外部身份的会话加进只准读的名单）。Unix 上有收紧手段的，真的经
//! 助手跑 `shell`：技能读得到、写不进，旁边的数据根读不到。

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 在 alice 家目录里放技能 `notes`，旁边放一份不给碰的；交回技能文件夹的真实位置。
fn put(home: &Home) -> PathBuf {
    let dir = home
        .root
        .account_dir(&alice_account())
        .join("skills")
        .join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: notes\ndescription: Take notes.\n---\n照这样记。\n",
    )
    .expect("写得进");
    std::fs::write(home.root.path().join("marker"), "secret\n").expect("写得进");
    std::fs::canonicalize(dir).expect("在")
}

/// 工作区 `work/` 的会话，沙盒能用、助手是 `helper`，场所 `venue`、属主是不是管理员照 `owner_is_admin`：调一次 `tools` 里的
/// `call`，交回会话。
async fn session(
    home: &Home,
    (tools, call): (&Catalog, Play),
    helper: &Path,
    (venue, owner_is_admin): (&str, bool),
) -> miyu_session::Handle {
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let script = Script::new([call, Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper.to_path_buf()),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        owner_is_admin,
        ..Lines::default()
    };
    let handle = home.create_full(&script, tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    handle
}

/// 执行命令的假工具调一次，交回它拿到的沙盒（没跑的没有）。
async fn spec_of(home: &Home, venue: (&str, bool)) -> Option<Arc<Sandboxed>> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let call = Play::calls(&[("run", "{}")]);
    session(home, (&tools, call), Path::new("miyu-sandbox"), venue).await;
    run.calls().into_iter().next().and_then(|call| call.sandbox)
}

#[tokio::test]
async fn the_skill_folder_is_revealed_in_the_spec() {
    let home = Home::outside_temp();
    let skill = put(&home);
    let sandboxed = spec_of(&home, ("local", true)).await.expect("带着沙盒");
    assert_eq!(
        sandboxed.spec.reveal,
        std::slice::from_ref(&skill),
        "本机的会话：单独放开只读"
    );
    assert_eq!(
        sandboxed.spec.hidden,
        [std::fs::canonicalize(home.root.path()).expect("在")],
        "数据根照旧藏着"
    );
    let home = Home::outside_temp();
    let skill = put(&home);
    let confined = spec_of(&home, ("qq:group:1", false)).await;
    if !miyu_sandbox::CONFINES_READS {
        assert!(confined.is_none(), "关不住读的平台上不跑");
        return;
    }
    let confined = confined.expect("带着沙盒");
    let read = confined.spec.read.clone().expect("只准读名单里的");
    assert!(
        read.contains(&skill),
        "外部身份的会话：加进只准读的名单：{read:?}"
    );
    assert!(confined.spec.reveal.is_empty());
}

/// 真的经助手跑 `shell`（Unix 上有收紧手段的）：技能读得到、写不进，旁边的数据根读不到。
#[cfg(unix)]
#[tokio::test]
async fn a_command_reads_the_skill_but_not_the_data_root() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let home = Home::outside_temp();
    let skill = put(&home).join("SKILL.md");
    let marker = home.root.path().join("marker");
    let command = format!(
        "head -n 2 '{0}'; \
         (echo x >> '{0}') 2>/dev/null && echo wrote-skill || echo blocked-skill; \
         cat '{1}' 2>/dev/null || echo blocked-data",
        skill.display(),
        marker.display()
    );
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let call = Play::calls(&[("shell", args.as_str())]);
    let handle = session(&home, (&tools, call), &helper, ("local", true)).await;
    let result = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有结果");
    assert_eq!(result.status, ToolStatus::Ok, "{result:?}");
    let output: String = result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    assert_eq!(output, "---\nname: notes\nblocked-skill\nblocked-data\n");
    assert!(
        std::fs::read_to_string(&skill)
            .expect("在")
            .ends_with("照这样记。\n"),
        "没写进去"
    );
}

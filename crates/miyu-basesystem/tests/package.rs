//! `package`（`docs/blueprint/tools/package.md`，施工 F-10 下）：访问类别 `install`；装的包文件夹报那个路径是读，卸不报；装、卸
//! 经端口，说装上了、卸掉了；核心拒了的照 `packages` 那几句说；问人之前看一眼：装的看文件夹、卸的看卸之前那一份，看不了的照
//! 拒绝交回；参数写错的、相对路径的不问端口；没有端口说做不了。给人看的说法中文、英文都换得出字。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::template::escape;
use miyu_kernel::tool::Access;
use miyu_tool::{Done, Looking, PackageConflict, PackageRefusal, PackagesPort};

use crate::support::{Site, check, human, readable, said, tool};

/// 假的端口：装、卸、看各交回给的那一份；记下被问了哪几样。
struct Port {
    answer: Result<Value, PackageRefusal>,
    asked: Mutex<Vec<String>>,
}

impl Port {
    fn new(answer: Result<Value, PackageRefusal>) -> Arc<Port> {
        Arc::new(Port {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    fn answered(&self, what: String) -> Looking<'_> {
        self.asked.lock().expect("没人拿着锁崩").push(what);
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("没人拿着锁崩").clone()
    }
}

impl PackagesPort for Port {
    fn list(&self) -> Looking<'_> {
        self.answered("list".to_string())
    }

    fn info(&self, package: String) -> Looking<'_> {
        self.answered(format!("info {package}"))
    }

    fn inspect(&self, path: PathBuf) -> Looking<'_> {
        self.answered(format!("inspect {}", path.display()))
    }

    fn install(&self, path: PathBuf, preset: Option<String>) -> Looking<'_> {
        self.answered(format!("install {} {preset:?}", path.display()))
    }

    fn enabling(&self, package: String) -> Looking<'_> {
        self.answered(format!("enabling {package}"))
    }

    fn enable(&self, package: String) -> Looking<'_> {
        self.answered(format!("enable {package}"))
    }

    fn removing(&self, package: String) -> Looking<'_> {
        self.answered(format!("removing {package}"))
    }

    fn remove(&self, package: String) -> Looking<'_> {
        self.answered(format!("remove {package}"))
    }
}

fn refusal(reason: &str, problem: Option<&str>, line: Option<u64>) -> PackageRefusal {
    PackageRefusal {
        reason: reason.to_string(),
        problem: problem.map(str::to_string),
        line,
        conflicts: Vec::new(),
    }
}

fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

fn absolute(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

async fn package(port: &Arc<Port>, args: Value) -> Done {
    let port = Arc::clone(port) as Arc<dyn PackagesPort>;
    Site::new()
        .done_with_packages("package", args, Some(port))
        .await
}

/// 问人之前看一眼；不看的是 `None`。
async fn preview(port: &Arc<Port>, args: Value) -> Option<Result<Value, Done>> {
    let port = Arc::clone(port) as Arc<dyn PackagesPort>;
    let call = Site::new().call_with_packages(args, Some(port));
    let tool = tool("package");
    let looked = tool.preview(call)?.await;
    Some(looked)
}

#[test]
fn installing_reads_the_folder_and_removing_touches_no_path() {
    let tool = tool("package");
    assert_eq!(tool.spec().access, Access::Install);
    let site = Site::new();
    let path = absolute("kit");
    let call = site.call_with_packages(json!({"install": path}), None);
    let targets = tool.targets(&call);
    assert_eq!(targets.len(), 1);
    assert_eq!(
        (targets[0].path.as_str(), targets[0].write),
        (path.as_str(), false)
    );
    assert_eq!(tool.asking(&call), [("install", path)]);
    let call = site.call_with_packages(json!({"remove": "kit"}), None);
    assert!(tool.targets(&call).is_empty());
    assert_eq!(tool.asking(&call), [("remove", "kit".to_string())]);
}

#[tokio::test]
async fn installing_and_removing_go_through_the_core() {
    let mut checked = Vec::new();
    let path = absolute("kit");
    let port = Port::new(Ok(json!({"package": "kit"})));
    let done = package(&port, json!({"install": path})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "Installed kit. Its tools and skills come in on the next turn.\n"
    );
    check(
        &mut checked,
        human(done),
        said("package/installed").with("package", "kit"),
    );
    let done = package(&port, json!({"remove": "kit"})).await;
    assert_eq!(text(&done), "Removed kit.\n");
    check(
        &mut checked,
        human(done),
        said("package/removed").with("package", "kit"),
    );
    let done = package(&port, json!({"enable": "kit"})).await;
    assert_eq!(
        text(&done),
        "Turned on kit. Its tools come in on the next turn.\n"
    );
    check(
        &mut checked,
        human(done),
        said("package/enabled").with("package", "kit"),
    );
    assert_eq!(
        port.asked(),
        [
            format!("install {path} None"),
            "remove kit".to_string(),
            "enable kit".to_string()
        ]
    );
    let refused = Port::new(Err(refusal("package_exists", None, None)));
    let done = package(&refused, json!({"install": path})).await;
    assert!(done.error);
    let shown = escape(&path);
    assert_eq!(
        text(&done),
        format!("Could not change {shown}: package_exists.\n")
    );
    check(&mut checked, human(done), said("package/failed"));
    let unknown = Port::new(Err(refusal("unknown_package", None, None)));
    let done = package(&unknown, json!({"remove": "kit"})).await;
    assert_eq!(text(&done), "No package named kit.\n");
    // 和装着的包撞了名：一处一行（施工 F-8 六补修），装回出厂的照编号说。
    let clash = Port::new(Err(PackageRefusal {
        conflicts: vec![
            PackageConflict {
                kind: "skill".to_string(),
                name: "pdf".to_string(),
                package: "kit".to_string(),
            },
            PackageConflict {
                kind: "tool".to_string(),
                name: "read".to_string(),
                package: "basesystem".to_string(),
            },
        ],
        ..refusal("package_conflict", None, None)
    }));
    let done = package(&clash, json!({"install": path})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!(
            "{shown} cannot be installed: these names are already in use.\n- skill pdf (in kit)\n- tool read (in basesystem)\n"
        )
    );
    check(&mut checked, human(done), said("package/failed"));
    readable(&checked, &["package"]);
}

#[tokio::test]
async fn asking_shows_what_the_core_would_do() {
    let path = absolute("kit");
    let port = Port::new(Ok(json!({"package": "kit", "files": 2})));
    let looked = preview(&port, json!({"install": path})).await;
    assert_eq!(looked, Some(Ok(json!({"package": "kit", "files": 2}))));
    let looked = preview(&port, json!({"remove": "kit"})).await;
    assert!(matches!(looked, Some(Ok(_))));
    let looked = preview(&port, json!({"enable": "kit"})).await;
    assert!(matches!(looked, Some(Ok(_))));
    assert_eq!(
        port.asked(),
        [
            format!("inspect {path}"),
            "removing kit".to_string(),
            "enabling kit".to_string()
        ]
    );
    let broken = Port::new(Err(refusal(
        "package_invalid",
        Some("Unknown table [x]"),
        Some(3),
    )));
    let Some(Err(done)) = preview(&broken, json!({"install": path})).await else {
        panic!("看不了的照拒绝交回");
    };
    assert!(done.error);
    let shown = escape(&path);
    assert_eq!(
        text(&done),
        format!("{shown} is not a valid package, line 3: Unknown table [x]\n")
    );
    assert!(preview(&port, json!({})).await.is_none(), "参数写错的不看");
}

#[tokio::test]
async fn wrong_arguments_never_reach_the_core() {
    let port = Port::new(Ok(json!({})));
    let relative = package(&port, json!({"install": "kit"})).await;
    assert_eq!(text(&relative), "path must be absolute.\n");
    let both = package(&port, json!({"install": absolute("a"), "remove": "a"})).await;
    assert_eq!(text(&both), "Give one of install, remove or enable.\n");
    let two = package(&port, json!({"remove": "a", "enable": "a"})).await;
    assert_eq!(text(&two), "Give one of install, remove or enable.\n");
    let neither = package(&port, json!({})).await;
    assert_eq!(text(&neither), "Give one of install, remove or enable.\n");
    assert!(port.asked().is_empty());
    let none = Site::new()
        .done_with_packages("package", json!({"remove": "kit"}), None)
        .await;
    assert!(none.error);
    assert_eq!(
        text(&none),
        "Packages cannot be installed or removed here.\n"
    );
}

/// 装的时候带上会话用的预设（施工 F-10 下下）：核心照它在那一份预设里打开装上的包。
#[tokio::test]
async fn installing_names_the_sessions_preset() {
    let port = Port::new(Ok(json!({"package": "kit"})));
    let path = absolute("kit");
    let mut call = Site::new().call_with_packages(
        json!({"install": path}),
        Some(Arc::clone(&port) as Arc<dyn PackagesPort>),
    );
    call.ids = Some(miyu_tool::CallIds {
        session: miyu_kernel::id::SessionId::parse("01a12f08-e382-70dc-942b-5deddc48d4da")
            .expect("合写法"),
        owner: miyu_kernel::id::AccountId::parse("alice").expect("合写法"),
        call: miyu_kernel::id::CallId::new(miyu_kernel::id::Seq::new(1).expect("合写法"), 1)
            .expect("合写法"),
        asked: None,
        preset: Some("dev".to_string()),
    });
    let done = tool("package")
        .run(call, miyu_tool::Progress::new(|_| {}))
        .await;
    assert!(!done.error);
    assert_eq!(port.asked(), [format!("install {path} Some(\"dev\")")]);
}

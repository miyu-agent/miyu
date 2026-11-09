//! 权限策略在阻塞线程里判（施工 4-9 再补四下）：判的时候跑异步任务的线程空着；问人时写出的 `rule`、`detail` 和
//! 样本 `docs/designs/samples/events/tool.approval_requested.jsonl` 是同一个样子。
//!
//! 场地不在系统的临时目录里（[`Home::outside_temp`]）：工作区 `work/`、假的家 `home/`。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

use serde_json::Value;

use miyu_kernel::event::{Body, Event, Level, Permission};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Call, Catalog, Done, Progress, Running, Spec, Target, Tool};

use crate::support::*;

/// 报路径时停住的工具：进来了说一声，等测试放它走，最多等六十秒；放走了没有记下来。
struct Slow {
    spec: Spec,
    entered: tokio::sync::mpsc::UnboundedSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    released: Arc<AtomicBool>,
}

impl Tool for Slow {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, _call: &Call) -> Vec<Target> {
        self.entered.send(()).expect("测试在等");
        let waited = self
            .release
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(Duration::from_secs(60));
        self.released.store(waited.is_ok(), Ordering::SeqCst);
        vec![Target {
            path: "a.txt".to_string(),
            write: false,
            itself: false,
        }]
    }

    fn run(&self, _call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async { Done::ok("read") })
    }
}

/// 在工作区 `work/` 里造一个会话：完全放开，只读 `read_only`。
async fn session(
    home: &Home,
    script: &Script,
    catalog: &Catalog,
    (attended, read_only): (bool, bool),
) -> miyu_session::Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    home.create_as(script, catalog, opening).await
}

#[tokio::test]
async fn the_async_thread_is_free_while_the_guard_judges() {
    let home = Home::outside_temp();
    let (entered_tx, mut entered) = tokio::sync::mpsc::unbounded_channel();
    let (release, release_rx) = mpsc::channel();
    let released = Arc::new(AtomicBool::new(false));
    let slow = Arc::new(Slow {
        spec: Spec {
            name: "slow".to_string(),
            description: "Stops while it names its paths.".to_string(),
            parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).expect("是 JSON"),
            access: Access::Read,
            clicall: false,
            result: None,
            plugin: None,
            role: None,
            scope: miyu_tool::Scope::Any,
        },
        entered: entered_tx,
        release: Mutex::new(release_rx),
        released: Arc::clone(&released),
    });
    let catalog = Catalog::new([slow as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("slow", "{}")]), Play::Says("好。")]);
    let handle = session(&home, &script, &catalog, (false, false)).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 工具在报路径：权限策略正在判。测试这边跑得到这里、放得了它，是因为判的时候异步线程空着。原来当场在 actor 里
    // 判，单线程的运行时整个卡住，要等工具自己等满六十秒。
    within("开始判", entered.recv()).await;
    release.send(()).expect("工具在等");
    until_turn_ends(&mut pushes).await;
    assert!(released.load(Ordering::SeqCst), "判的时候异步线程该空着");
}

/// 一个 JSON 的样子：键照原样，值只留种类，数组看每一个。
fn shape(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), shape(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(shape).collect()),
        Value::String(_) => Value::from("string"),
        Value::Bool(_) => Value::from("bool"),
        Value::Number(_) => Value::from("number"),
        Value::Null => Value::Null,
    }
}

/// 一条确认请求里的 `rule`、`detail`。
fn rule_and_detail(event: &Event) -> (Value, Value) {
    let Body::ApprovalRequested(request) = &event.body else {
        panic!("该是确认请求：{event:?}");
    };
    let read = |raw: &Option<RawJson>| -> Value {
        serde_json::from_str(raw.as_ref().expect("写了").get()).expect("是 JSON")
    };
    (read(&request.rule), read(&request.detail))
}

/// A16 起报路径的调用问人的只剩只读时读别的程序的私密目录（施工 LV-1）：照它问。
#[tokio::test]
async fn asking_writes_what_the_sample_shows() {
    let home = Home::outside_temp();
    let private = home.home.join(".ssh/config");
    std::fs::create_dir_all(private.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&private, "Host *\n").expect("写得进");
    let read = Fake::new("read", Access::Read, Act::Echo);
    let catalog = Catalog::new([read as Arc<dyn Tool>]).expect("合写法");
    let args = serde_json::json!({ "path": private.to_string_lossy() }).to_string();
    let script = Script::new([Play::calls(&[("read", &args)])]);
    let handle = session(&home, &script, &catalog, (true, true)).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let id = handle.id().clone();
    until_logged(&home, &id, |log| {
        kinds(log).contains(&"tool.approval_requested")
    })
    .await;
    let asked = home
        .log(&id)
        .into_iter()
        .find(|event| matches!(event.body, Body::ApprovalRequested(_)))
        .expect("有一条确认请求");
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/events/tool.approval_requested.jsonl");
    let line = std::fs::read_to_string(&sample).expect("有样本");
    let drawn = Event::from_line(line.trim_end()).expect("样本读得进来");
    let (rule, detail) = rule_and_detail(&asked);
    let (drawn_rule, drawn_detail) = rule_and_detail(&drawn);
    // 键、值的种类和样本一样；读的、私密目录里的也一样（施工 4-9 再补四下：样本原来是另一种写法，只比来回没拦住）。
    assert_eq!(shape(&rule), shape(&drawn_rule), "{rule}");
    assert_eq!(shape(&detail), shape(&drawn_detail), "{detail}");
    assert_eq!(
        detail["paths"][0]["write"],
        drawn_detail["paths"][0]["write"]
    );
    assert_eq!(detail["paths"][0]["zone"], drawn_detail["paths"][0]["zone"]);
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

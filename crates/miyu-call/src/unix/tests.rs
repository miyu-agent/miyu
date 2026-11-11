//! 端点本身（施工 M-3，设计 39 第五节第 2、3、6 条）：真的套接字，测试进程自己当「这条命令」（记下它自己的进程组）。

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use miyu_kernel::event::NestedStatus;
use miyu_kernel::id::JobId;
use miyu_tool::{Calling, Nested, Schema, Settled};
use serde_json::{Value, json};

use super::*;

/// 出厂的字。
fn texts() -> Arc<Texts> {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Texts::load(&resources).expect("出厂的字读得出")
}

/// 调什么都成、交回 `ok`、记下调了几次的假端口；`hold` 的一直等着不答。
#[derive(Default)]
struct Port {
    calls: Mutex<Vec<String>>,
    hold: bool,
}

impl NestedPort for Port {
    fn call(&self, name: String, _args: String) -> Calling<'_> {
        self.calls.lock().unwrap().push(name);
        Box::pin(async move {
            if self.hold {
                std::future::pending::<()>().await;
            }
            Nested::Settled(Settled {
                text: "ok".to_string(),
                ..Settled::without_result(NestedStatus::Ok, None)
            })
        })
    }

    /// 会话里的 `miyu pkg`（施工 BS-1 中）：记成 `pkg:<名字>`，交回 `pkg ok`。
    fn command(&self, name: String, _args: String) -> Calling<'_> {
        self.calls.lock().unwrap().push(format!("pkg:{name}"));
        Box::pin(async move {
            Nested::Settled(Settled {
                text: "pkg ok".to_string(),
                ..Settled::without_result(NestedStatus::Ok, None)
            })
        })
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        Vec::new()
    }

    fn schema(&self, _name: &str) -> Option<Schema> {
        None
    }
}

/// 开一个端点，记下测试进程自己的进程组。
fn open(port: Arc<Port>) -> Channel {
    let channel = Channel::open(port, texts(), Kin::command()).expect("开得了");
    let own = rustix::process::getpgid(None).expect("查得到");
    channel
        .binder()
        .expect("命令的有")
        .bind(u32::try_from(own.as_raw_nonzero().get()).unwrap());
    channel
}

/// 连上去发一帧 `body`（先写长度 `len`），读回答；对方断开、没答的是空的。
async fn ask_raw(path: &Path, len: u32, body: &[u8]) -> Option<Value> {
    let mut stream = UnixStream::connect(path).await.ok()?;
    stream.write_u32(len).await.ok()?;
    stream.write_all(body).await.ok()?;
    let len = stream.read_u32().await.ok()?;
    let mut reply = vec![0u8; len as usize];
    stream.read_exact(&mut reply).await.ok()?;
    serde_json::from_slice(&reply).ok()
}

async fn ask(path: &Path, request: &Value) -> Option<Value> {
    let body = request.to_string();
    ask_raw(path, u32::try_from(body.len()).unwrap(), body.as_bytes()).await
}

fn request(token: &str, v: u32) -> Value {
    json!({"v": v, "op": "call", "tool": "web_search", "args": "{}", "token": token})
}

#[tokio::test]
async fn the_right_token_from_this_group_is_answered() {
    let port = Arc::new(Port::default());
    let channel = open(Arc::clone(&port));
    let reply = ask(&channel.path, &request(&channel.token, VERSION))
        .await
        .expect("答了");
    assert_eq!(reply, json!({"code": 0, "stdout": "ok\n", "stderr": ""}));
    assert_eq!(*port.calls.lock().unwrap(), ["web_search"]);
}

/// `op: "pkg"`（施工 BS-1 中）：走端口的 `command`，不走 `call`。
#[tokio::test]
async fn miyu_pkg_goes_the_command_way() {
    let port = Arc::new(Port::default());
    let channel = open(Arc::clone(&port));
    let pkg = json!({"v": VERSION, "op": "pkg", "tool": "packages", "args": "{}", "token": channel.token});
    let reply = ask(&channel.path, &pkg).await.expect("答了");
    assert_eq!(
        reply,
        json!({"code": 0, "stdout": "pkg ok\n", "stderr": ""})
    );
    assert_eq!(*port.calls.lock().unwrap(), ["pkg:packages"]);
}

#[tokio::test]
async fn a_wrong_token_or_version_is_refused_without_calling() {
    let port = Arc::new(Port::default());
    let channel = open(Arc::clone(&port));
    let wrong = ask(&channel.path, &request("0000", VERSION))
        .await
        .expect("答了");
    assert_eq!(wrong["code"], 2);
    assert_eq!(
        wrong["stderr"],
        "miyu call only works from processes started by the command Miyu ran.\n"
    );
    let old = ask(&channel.path, &request(&channel.token, VERSION + 1))
        .await
        .expect("答了");
    assert_eq!(old["code"], 2);
    assert!(
        old["stderr"]
            .as_str()
            .unwrap()
            .starts_with("This miyu does not match")
    );
    let garbage = ask_raw(&channel.path, 3, b"{x}").await.expect("答了");
    assert_eq!(garbage["stderr"], "Bad request from miyu call.\n");
    assert!(port.calls.lock().unwrap().is_empty(), "一次都没调");
}

/// 超过上限的帧：只看长度就断开，不等后面的字（等的话，对方报个大长度就能把这条连接一直占着）。
#[tokio::test]
async fn an_oversized_frame_is_dropped_at_once() {
    let port = Arc::new(Port::default());
    let channel = open(Arc::clone(&port));
    let mut stream = UnixStream::connect(&channel.path).await.expect("连得上");
    stream.write_u32(MAX_REQUEST + 1).await.expect("写得进");
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut byte))
        .await
        .expect("当场断开，不等后面的字");
    assert_eq!(read.ok(), Some(0), "读到结尾");
    assert!(port.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn too_many_open_connections_are_dropped() {
    let port = Arc::new(Port {
        hold: true,
        ..Port::default()
    });
    let channel = open(Arc::clone(&port));
    let body = request(&channel.token, VERSION).to_string();
    let mut held = Vec::new();
    for _ in 0..MAX_OPEN {
        let mut stream = UnixStream::connect(&channel.path).await.expect("连得上");
        stream
            .write_u32(u32::try_from(body.len()).unwrap())
            .await
            .unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        held.push(stream);
    }
    // 都在等着：数到 32 个调用。
    let started = std::time::Instant::now();
    while port.calls.lock().unwrap().len() < MAX_OPEN && started.elapsed() < Duration::from_secs(10)
    {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(port.calls.lock().unwrap().len(), MAX_OPEN);
    assert_eq!(
        ask(&channel.path, &json!({})).await,
        None,
        "第 33 个直接断开"
    );
}

#[tokio::test]
async fn dropping_it_closes_the_endpoint() {
    let channel = open(Arc::new(Port::default()));
    let path = channel.path.clone();
    drop(channel);
    assert!(!path.exists(), "套接字删掉了");
}

#[tokio::test]
async fn the_sandbox_lets_the_command_reach_this_socket_only() {
    let channel = open(Arc::new(Port::default()));
    let base = Sandboxed {
        helper: "/helper".into(),
        spec: miyu_sandbox::Spec {
            write: Vec::new(),
            hidden: Vec::new(),
            read: Some(vec!["/usr".into()]),
            reveal: Vec::new(),
            sockets: Vec::new(),
        },
        env: Vec::new(),
        uncached: false,
    };
    let sandboxed = channel.sandboxed(&base);
    assert_eq!(sandboxed.spec.sockets, std::slice::from_ref(&channel.path));
    let read = sandboxed.spec.read.expect("还是名单");
    assert!(read.contains(&"/usr".into()));
    assert!(read.len() > 1, "放开读 `miyu` 和核心的程序：{read:?}");
    let mut env = vec![(OsString::from("PATH"), OsString::from("/usr/bin"))];
    channel.env(&mut env);
    let get = |name: &str| {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };
    assert_eq!(
        get("MIYU_CALL_SOCK"),
        Some(channel.path.to_string_lossy().into_owned())
    );
    assert_eq!(get("MIYU_CALL_TOKEN").map(|token| token.len()), Some(32));
    assert!(
        get("PATH").unwrap().ends_with(":/usr/bin"),
        "放 `miyu` 的目录排在最前"
    );
}

#[tokio::test]
async fn the_core_s_own_program_under_a_hidden_place_is_let_through_read_only() {
    // 施工 5-15 下再修：私密目录包住了核心装的位置，脚本里的 `miyu call` 找不到核心的程序。只在藏住了它的时候放开，
    // 能写的里面的（在自己的工作树里开发 Miyu）不另挂，不然换不掉它。
    let channel = open(Arc::new(Port::default()));
    let exe = std::env::current_exe().expect("找得到");
    let place = exe.parent().expect("有上一层").to_path_buf();
    let spec = |write: Vec<PathBuf>| miyu_sandbox::Spec {
        write,
        hidden: vec![place.clone()],
        read: None,
        reveal: Vec::new(),
        sockets: Vec::new(),
    };
    let base = |write| Sandboxed {
        helper: "/helper".into(),
        spec: spec(write),
        env: Vec::new(),
        uncached: false,
    };
    let hidden = channel.sandboxed(&base(Vec::new()));
    assert_eq!(hidden.spec.reveal, vec![exe.clone()]);
    let writable = channel.sandboxed(&base(vec![place.clone()]));
    assert!(
        writable.spec.reveal.is_empty(),
        "{:?}",
        writable.spec.reveal
    );
    let mut elsewhere = base(Vec::new());
    elsewhere.spec.hidden = vec!["/nowhere".into()];
    assert!(channel.sandboxed(&elsewhere).spec.reveal.is_empty());
}

/// 清留下的目录（施工 M-3 上，WC-1 挪到 `leftovers.rs`）：自己这个进程号的留着，等它退了、收掉了的子进程的整个删掉。真的问系统。
#[test]
fn directories_of_gone_cores_are_swept() {
    let temp = std::env::temp_dir().join(format!("miyu-sweep-test-{}", std::process::id()));
    let mut child = std::process::Command::new("true").spawn().expect("起得来");
    let gone = child.id();
    child.wait().expect("等得到");
    let mine = temp.join(format!("miyu-call-{}", std::process::id()));
    let left = temp.join(format!("miyu-call-{gone}"));
    for dir in [&mine, &left] {
        std::fs::create_dir_all(dir.join("bin")).expect("建得了");
    }
    sweep(&temp);
    let (kept, swept) = (mine.is_dir(), !left.exists());
    std::fs::remove_dir_all(&temp).expect("删得掉");
    assert!(kept, "自己的留着");
    assert!(swept, "进程不在了的删掉");
}

/// 扩展的那一种（施工 XG-1 下，设计 63 第六节第 2 条）：测试进程是它父进程的子孙，答；不是另一个进程的子孙，照「不是这条命令的
/// 进程」拒，那一次一回都没调。没有记进程组的那一头。
#[tokio::test]
async fn an_extension_s_endpoint_answers_its_descendants_only() {
    let port = Arc::new(Port::default());
    let parent = std::os::unix::process::parent_id();
    let ours = Channel::open(port.clone(), texts(), Kin::descendant_of(parent)).expect("开得了");
    assert!(ours.binder().is_none());
    let answered = ask(&ours.path, &request(&ours.token, VERSION))
        .await
        .expect("答了");
    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .expect("起得来");
    let theirs =
        Channel::open(port.clone(), texts(), Kin::descendant_of(child.id())).expect("开得了");
    let refused = ask(&theirs.path, &request(&theirs.token, VERSION))
        .await
        .expect("答了");
    child.kill().expect("收得掉");
    child.wait().expect("等得到");
    assert_eq!(answered["code"], 0);
    assert_eq!(refused["code"], 2);
    assert_eq!(
        refused["stderr"],
        "miyu call only works from processes started by the command Miyu ran.\n"
    );
    assert_eq!(*port.calls.lock().unwrap(), ["web_search"]);
}

/// 交给扩展的那一格（`tool.call` 的 `channel`）：和设进命令的环境变量一样。
#[tokio::test]
async fn the_extension_is_handed_the_same_endpoint_token_and_bin() {
    let channel =
        Channel::open(Arc::new(Port::default()), texts(), Kin::descendant_of(1)).expect("开得了");
    let handed = channel.handed().expect("交得出");
    let mut env = vec![(OsString::from("PATH"), OsString::from("/usr/bin"))];
    channel.env(&mut env);
    let get = |name: &str| {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };
    assert_eq!(Some(handed.sock.clone()), get("MIYU_CALL_SOCK"));
    assert_eq!(Some(handed.token.clone()), get("MIYU_CALL_TOKEN"));
    // 端点名不带令牌的任何一段（施工 XG-1补2）。
    assert!(!handed.sock.contains(&handed.token[..8]));
    assert!(!handed.sock.contains(&handed.token[16..24]));
    let bin = handed.bin.clone().expect("建得了放 `miyu` 的目录");
    assert_eq!(get("PATH"), Some(format!("{bin}:/usr/bin")));
    assert_eq!(
        serde_json::to_value(&handed).unwrap(),
        json!({"sock": handed.sock, "token": handed.token, "bin": bin})
    );
}

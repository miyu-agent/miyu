//! 测试用的假工具（施工 4-2）：规格照给的，执行照剧本。只在 `testkit` 开关打开时编进去：工具目录自己的
//! 测试、会话和协议端点的测试都用它，在各自的 dev-dependencies 里打开（照会话的剧本端口，施工 3-8 上）。
//! 另有换了名字的一件 [`Renamed`]：造改名以前的核心的目录（施工 7-5 再补）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_kernel::event::{JobKind, JobStarted, Question};
use miyu_kernel::id::MediaType;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use tokio::sync::Barrier;

use crate::{Call, Done, Effect, Picture, Progress, Running, Spec, Stop, Target, Tool, Venues};

mod held;
mod nested;
mod renamed;

pub use held::Held;
pub use renamed::Renamed;

/// 假工具跑起来做什么。
#[derive(Debug, Clone)]
pub enum Act {
    /// 回一句成功：工作目录和参数原文，写成 `cwd=<目录> args=<参数>`。
    Echo,
    /// 回一句出错。
    Fails(&'static str),
    /// 回一句成功 `ended`，要结束这一轮（施工 4-14）。
    Ends,
    /// 先推这几段输出，再回一句成功 `pushed`。
    Pushes(&'static [&'static str]),
    /// 停住不回，等被叫停（丢掉）。不看旗。
    Holds,
    /// 等叫它停（旗举起来），停在改之前，交回 `stopped`（施工 4-9 再补一）。
    Stops,
    /// 等叫它停，看到旗时已经改完了：照常回一句成功 `wrote`（施工 4-9 再补一）。
    Finishes,
    /// 等凑齐一起跑的（`Barrier` 的人数）再回一句成功 `met`：一起派的才走得完。
    Meets(Arc<Barrier>),
    /// 工具自己的 bug：一跑就 panic。
    Panics,
    /// 回一句成功 `shown`，再交一张 2×1 的 PNG，字节是给的这些（施工 4-13）。
    Shows(&'static [u8]),
    /// 把这条假的后台命令交给任务端口（施工 7-3）：交上了回一句成功 `started <编号>`，报 `job.started`（后台命令，标题
    /// `fake`）；交不上、没有端口的回一句出错。
    Background(Arc<Held>),
    /// 经提问的端口问这组题（施工 D-2）：答了回一句成功 `answered <回答>`（回答写成 JSON）；没答到就了结的交回 `stopped`；
    /// 没有端口的回一句出错。
    Asks(Vec<Question>),
    /// 回一句成功，带给脚本的结构化结果（施工 M-2）：字、`data` 的 JSON 原文。
    Gives(&'static str, &'static str),
    /// 经脚本调工具的端口一件一件调（施工 M-2）：都了结了回一句成功，一行一件，写成 `<工具> <结局>`；跑完的再写字和
    /// `data`，丢了图的写 `images dropped`，拒绝的写原因。没有端口的回一句出错。
    Calls(Vec<(&'static str, &'static str)>),
    /// 照 [`Act::Background`] 放到后台，端口挂到那个任务下面，当场在旁边调一次这件工具（施工 M-3）：不等它答就交回结果，
    /// 所以调用到的时候任务的 `job.started` 还没记进日志。答回来的照 [`Act::Calls`] 的写法记进第三格。第四格是假的，结果里
    /// 不报这个任务（像调用被打断、任务没交上）：等着的那一次再也等不到。最后一格是在旁边调之前先等几毫秒：等得够久，调用
    /// 到的时候结果已经记进日志了。
    BackgroundCalls(Arc<Held>, &'static str, Arc<Mutex<Vec<String>>>, bool, u64),
}

/// 一件假工具。
#[derive(Debug)]
pub struct Fake {
    spec: Spec,
    act: Act,
    venues: Option<Venues>,
    display_name: BTreeMap<String, String>,
    file: Option<PathBuf>,
    calls: Mutex<Vec<Call>>,
    dropped: Arc<AtomicUsize>,
    /// 声明自己跑命令、只在有后台时给的参数（施工 BS-1 上，[`Fake::commanding`]）。
    commands: Option<Option<&'static str>>,
}

impl Fake {
    /// 叫 `name` 的假工具：参数格式是 `{"type":"object"}`，说明是 `The <name> tool.`。
    ///
    /// # Panics
    ///
    /// 实际不会：参数格式是写死的 JSON。
    pub fn new(name: &str, access: Access, act: Act) -> Arc<Fake> {
        Fake::with_parameters(name, access, r#"{"type":"object"}"#, act)
    }

    /// 参数格式照 `parameters` 的原文。
    ///
    /// # Panics
    ///
    /// `parameters` 不是 JSON。
    pub fn with_parameters(name: &str, access: Access, parameters: &str, act: Act) -> Arc<Fake> {
        Fake::from_spec(
            Spec {
                name: name.to_string(),
                description: format!("The {name} tool."),
                parameters: serde_json::from_str::<RawJson>(parameters).expect("参数格式是 JSON"),
                access,
                need_load: false,
                clicall: false,
                result: None,
                plugin: None,
            },
            act,
        )
    }

    /// 规格照 `spec`。
    pub fn from_spec(spec: Spec, act: Act) -> Arc<Fake> {
        Arc::new(Fake {
            spec,
            act,
            venues: None,
            display_name: BTreeMap::new(),
            file: None,
            calls: Mutex::new(Vec::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
            commands: None,
        })
    }

    /// 像 `shell` 那样声明自己跑命令的假工具（施工 BS-1 上：核心照声明认，不看名字），`background` 是只在有后台时给的参数。
    pub fn commanding(
        name: &str,
        access: Access,
        background: Option<&'static str>,
        act: Act,
    ) -> Arc<Fake> {
        Fake::commanding_with(name, access, r#"{"type":"object"}"#, background, act)
    }

    /// 同 [`Fake::commanding`]，参数格式照 `parameters` 的原文。
    ///
    /// # Panics
    ///
    /// `parameters` 不是 JSON。
    pub fn commanding_with(
        name: &str,
        access: Access,
        parameters: &str,
        background: Option<&'static str>,
        act: Act,
    ) -> Arc<Fake> {
        let mut fake = Fake::with_parameters(name, access, parameters, act);
        if let Some(fake) = Arc::get_mut(&mut fake) {
            fake.commands = Some(background);
        }
        fake
    }

    /// 给 `venues` 那几种会话的假工具，像提供者的（施工 O-2 上）。
    pub fn in_venues(name: &str, access: Access, venues: Venues, act: Act) -> Arc<Fake> {
        let mut fake = Fake::new(name, access, act);
        if let Some(fake) = Arc::get_mut(&mut fake) {
            fake.venues = Some(venues);
        }
        fake
    }

    /// 带给人看的名字的假工具，像脚本、MCP 的、扩展的（施工 9-8 六补）：`display_name` 是语言到名字。
    pub fn named(name: &str, access: Access, display_name: &[(&str, &str)], act: Act) -> Arc<Fake> {
        let mut fake = Fake::new(name, access, act);
        if let Some(fake) = Arc::get_mut(&mut fake) {
            fake.display_name = display_name
                .iter()
                .map(|(language, name)| ((*language).to_string(), (*name).to_string()))
                .collect();
        }
        fake
    }

    /// 脚本里能调的假工具（施工 M-2）：规格的 `clicall` 是真的。
    pub fn callable(name: &str, access: Access, act: Act) -> Arc<Fake> {
        let mut fake = Fake::new(name, access, act);
        if let Some(fake) = Arc::get_mut(&mut fake) {
            fake.spec.clicall = true;
        }
        fake
    }

    /// 带着一个文件的假工具，像脚本（施工 X-37 补）：管理员放行出沙盒时照这个文件的真实位置判。
    pub fn with_file(name: &str, access: Access, file: PathBuf, act: Act) -> Arc<Fake> {
        let mut fake = Fake::new(name, access, act);
        if let Some(fake) = Arc::get_mut(&mut fake) {
            fake.file = Some(file);
        }
        fake
    }

    /// 交给它的每一次调用，照先后。
    pub fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 有几次没跑完就被丢掉了：叫停了的。
    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::Acquire)
    }
}

/// 跑完以前被丢掉，就数一次。
struct Guard {
    dropped: Arc<AtomicUsize>,
    finished: bool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            self.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }
}

/// 一次假的调用（施工 O-2 上）：参数照 `args` 的原文，工作目录是 `/`，别的端口、编号都没有。
pub fn call(args: &str) -> Call {
    Call {
        args: args.to_string(),
        cwd: "/".to_string(),
        home: None,
        data_root: None,
        seen: Arc::default(),
        stop: Stop::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: None,
        knowledge: None,
        packages: None,
        goals: None,
        nested: None,
        ids: None,
        settings: None,
        checked: None,
        caller: None,
    }
}

impl Tool for Fake {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn runs_commands(&self) -> bool {
        self.commands.is_some()
    }

    fn background_parameter(&self) -> Option<&'static str> {
        self.commands.flatten()
    }

    fn venues(&self) -> Option<Venues> {
        self.venues
    }

    fn display_name(&self) -> BTreeMap<String, String> {
        self.display_name.clone()
    }

    /// 参数里的 `path`（一条）、`paths`（几条）就是要碰的路径；访问类别是写的，就是写（施工 4-3 下）。
    fn targets(&self, call: &Call) -> Vec<Target> {
        let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.args) else {
            return Vec::new();
        };
        let write = self.spec.access.writes();
        let one = args.get("path").and_then(serde_json::Value::as_str);
        let many = args
            .get("paths")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str);
        one.into_iter()
            .chain(many)
            .map(|path| Target {
                path: path.to_string(),
                write,
                itself: false,
            })
            .collect()
    }

    /// 参数里 `outside_sandbox` 是真的，就报要在沙盒外跑（施工 D-4）。
    fn outside_sandbox(&self, call: &Call) -> bool {
        serde_json::from_str::<serde_json::Value>(&call.args).is_ok_and(|args| {
            args.get("outside_sandbox")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
    }

    fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// 参数里有字的 `command`、`title`，照原样交给说明（施工 D-4）。
    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.args) else {
            return Vec::new();
        };
        ["command", "title"]
            .into_iter()
            .filter_map(|key| {
                args.get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(|value| (key, value.to_string()))
            })
            .collect()
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call.clone());
        let act = self.act.clone();
        let guard = Guard {
            dropped: Arc::clone(&self.dropped),
            finished: false,
        };
        Box::pin(async move {
            // 整个挪进来：只写 `guard.finished` 的话，闭包只捕获那一格，守卫在 `run` 返回时就被丢掉了。
            let mut guard = guard;
            let done = match act {
                Act::Echo => Done::ok(format!("cwd={} args={}", call.cwd, call.args)),
                Act::Fails(text) => Done::error(text),
                Act::Ends => Done::ok("ended").ending(),
                Act::Pushes(pieces) => {
                    for piece in pieces {
                        progress.push(*piece);
                    }
                    Done::ok("pushed")
                }
                Act::Holds => std::future::pending::<Done>().await,
                Act::Stops => {
                    raised(&call.stop).await;
                    Done::stopped()
                }
                Act::Finishes => {
                    raised(&call.stop).await;
                    Done::ok("wrote")
                }
                Act::Meets(barrier) => {
                    barrier.wait().await;
                    Done::ok("met")
                }
                Act::Panics => panic!("假工具自己的 bug"),
                Act::Shows(bytes) => Done::ok("shown").image(Picture {
                    bytes: bytes.to_vec(),
                    media_type: MediaType::parse("image/png").expect("写法对"),
                    width: 2,
                    height: 1,
                }),
                Act::Background(held) => background(&call, &held),
                Act::Asks(questions) => match call.questions.clone() {
                    Some(port) => match port.ask(questions).await {
                        Some(answers) => Done::ok(format!(
                            "answered {}",
                            serde_json::to_string(&answers).expect("回答写得成 JSON")
                        )),
                        None => Done::stopped(),
                    },
                    None => Done::error("no one to ask"),
                },
                Act::Gives(text, data) => {
                    Done::ok(text).data(serde_json::from_str(data).expect("data 是 JSON"))
                }
                Act::Calls(calls) => nested::calls(&call, calls).await,
                Act::BackgroundCalls(held, name, heard, report, wait) => {
                    let mut done = background(&call, &held);
                    nested::aside(&call, &done, name, heard, wait);
                    if !report {
                        done.effects.clear();
                    }
                    done
                }
            };
            guard.finished = true;
            done
        })
    }
}

/// 交给任务端口：交上了报 `job.started`。
fn background(call: &Call, held: &Arc<Held>) -> Done {
    let Some(port) = &call.jobs else {
        return Done::error("no job port");
    };
    match port.start(held.background()) {
        Ok(job) => Done::ok(format!("started {job}")).effect(Effect::JobStarted(JobStarted {
            foreground: false,
            fork: false,
            job,
            what: JobKind::Command,
            title: "fake".to_string(),
            session: None,
        })),
        Err(error) => Done::error(error.to_string()),
    }
}

/// 等到旗举起来。
async fn raised(stop: &Stop) {
    while !stop.stopped() {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

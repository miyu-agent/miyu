//! 几份测试共用的：假端口、调一次工具、拆结果。端口是假的：真的在协议端点里测。

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::Block;
use miyu_kernel::id::{AccountId, CallId, Seq, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_tool::{
    CallIds, Done, Progress, ScheduleAnswer, ScheduleAnswering, ScheduleAsk, SchedulePort,
    ScheduleRefused, Scheduled, Tool,
};

/// 测试里的会话。
pub const SESSION: &str = "01a12f08-e382-70dc-942b-5deddc48d4da";

/// 一个假端口：记下在哪个会话上要做什么，照预先定好的交回。
pub struct Fake {
    asked: Mutex<Vec<(SessionId, ScheduleAsk)>>,
    answer: Result<ScheduleAnswer, ScheduleRefused>,
}

impl Fake {
    pub fn answering(answer: Result<ScheduleAnswer, ScheduleRefused>) -> Arc<Fake> {
        Arc::new(Fake {
            asked: Mutex::default(),
            answer,
        })
    }

    pub fn asked(&self) -> Vec<(SessionId, ScheduleAsk)> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SchedulePort for Fake {
    fn schedule(&self, session: &SessionId, ask: ScheduleAsk) -> ScheduleAnswering<'_> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session.clone(), ask));
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

/// 两个包的工具，照名字取。
pub fn tool(name: &str) -> Arc<dyn Tool> {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut all = miyu_schedule::schedule_tools(&resources).expect("读得出");
    all.extend(miyu_schedule::loop_tools(&resources).expect("读得出"));
    all.into_iter()
        .find(|tool| tool.spec().name == name)
        .expect("有这件")
}

/// 用假端口 `port`（没有的是这个会话用不了）调一次 `name`。
pub async fn run(name: &str, port: Option<&Arc<Fake>>, args: serde_json::Value) -> Done {
    let mut call = miyu_tool::testkit::call(&args.to_string());
    call.schedules = port.map(|port| Arc::clone(port) as Arc<dyn SchedulePort>);
    call.ids = Some(ids());
    tool(name).run(call, Progress::new(|_| {})).await
}

/// 测试里这次调用的编号。
pub fn ids() -> CallIds {
    CallIds {
        session: session(),
        owner: AccountId::parse("alice").expect("合写法"),
        call: CallId::new(Seq::new(1).expect("合写法"), 1).expect("合写法"),
        asked: None,
        preset: None,
    }
}

pub fn session() -> SessionId {
    SessionId::parse(SESSION).expect("合写法")
}

/// 一项：下一次在 2026-10-27 09:00（UTC）。
pub fn item(id: &str, when: &str, prompt: &str) -> Scheduled {
    Scheduled {
        id: id.to_string(),
        when: when.to_string(),
        prompt: prompt.to_string(),
        recurring: true,
        kind: None,
        next: Some(at("2026-10-27T09:00:00.000Z")),
        paused: false,
    }
}

pub fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

/// 给她看的字。
pub fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 给人看的说法：编号和字段。
pub fn said(done: &Done) -> (String, Vec<(String, String)>) {
    let said = done.human.as_ref().expect("有给人看的说法");
    let fields = said
        .fields
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    (said.key.clone(), fields)
}

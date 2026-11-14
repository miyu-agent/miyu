//! 扩展驱动这一支（施工 MD-2，设计 66 第五、八节）：假端口按剧本交增量、回答，直接走 [`exchange`]。正常一段；交了一半断了；
//! 乱序的增量；没动静（空闲时限、总时限）；被叫停；回答写坏了、被拒；连不上的带等多久；回答之前到的增量一条不丢；别家的私有数据
//! 不交、自己家的照交；看得了图的交图。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;

use miyu_drivers::{Call, Inputs};
use miyu_http::Proxy;
use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::{Block, Reasoning, Text};
use miyu_kernel::id::{ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;

use super::super::driver_port::{Broken, Declared, DriverPort, NotYet, Pending};
use super::super::exchange::Learn;
use super::super::shared::ModelData;
use super::*;
use crate::route::base::Speaker;

mod hosted;

/// 一次调用怎么答。
#[derive(Debug)]
enum Plan {
    /// 交这几份增量（每份隔 `gap`），再回答。
    Answer(Vec<Value>, Duration, Value),
    /// 交这几份增量，就不动了。
    Hang(Vec<Value>),
    /// 交这几份增量，连接断了。
    Gone(Vec<Value>),
    /// 回 JSON-RPC 的错误。
    Refuse,
}

/// 假端口：照剧本答，记下交来的参数、哪几次被丢下了（没答完就不要了：真端口这时发 `driver.cancel`）。
#[derive(Debug, Default)]
struct Fake {
    plans: Mutex<VecDeque<Plan>>,
    params: Mutex<Vec<Value>>,
    dropped: Arc<Mutex<Vec<String>>>,
    not_yet: Option<NotYet>,
}

/// 在路上的一次：没答完就被丢下的记一笔。
struct Live {
    call: String,
    dropped: Arc<Mutex<Vec<String>>>,
    done: bool,
}

impl Drop for Live {
    fn drop(&mut self) {
        if !self.done {
            self.dropped.lock().expect("锁").push(self.call.clone());
        }
    }
}

impl DriverPort for Fake {
    fn present(&self, _: &DriverRef) -> Result<Declared, String> {
        Ok(Declared {
            key: DriverKey::Optional,
            images: true,
        })
    }

    fn call(
        &self,
        _: &DriverRef,
        call: &str,
        params: String,
        deltas: mpsc::UnboundedSender<Value>,
    ) -> Result<Pending, NotYet> {
        if let Some(not_yet) = self.not_yet {
            return Err(not_yet);
        }
        let params: Value = serde_json::from_str(&params).expect("参数是 JSON");
        assert_eq!(params["call"], call);
        self.params.lock().expect("锁").push(params);
        let plan = self.plans.lock().expect("锁").pop_front().expect("有剧本");
        let live = Live {
            call: call.to_string(),
            dropped: Arc::clone(&self.dropped),
            done: false,
        };
        Ok(Box::pin(async move {
            // 整个搬进来（只用 `live.done` 的话闭包只捕获那一格，`Live` 当场就被丢下了）。
            let mut live = live;
            let (batches, gap) = match &plan {
                Plan::Answer(batches, gap, _) => (batches.clone(), *gap),
                Plan::Hang(batches) | Plan::Gone(batches) => (batches.clone(), Duration::ZERO),
                Plan::Refuse => (Vec::new(), Duration::ZERO),
            };
            for batch in batches {
                tokio::time::sleep(gap).await;
                drop(deltas.send(batch));
            }
            let answered = match plan {
                Plan::Answer(_, _, answer) => Ok(answer),
                Plan::Hang(_) => std::future::pending().await,
                Plan::Gone(_) => Err(Broken::Gone),
                Plan::Refuse => Err(Broken::Refused("no such method".to_string())),
            };
            live.done = true;
            answered
        }))
    }
}

fn hello() -> Vec<Value> {
    vec![
        json!([{"start": {"index": 0, "kind": "text"}}, {"text": {"index": 0, "text": "he"}}]),
        json!([{"text": {"index": 0, "text": "llo"}}, {"end": {"index": 0}}]),
    ]
}

fn usage() -> Value {
    json!({"usage": {"uncached": 10, "cache_read": 2, "cache_write": 0, "output": 3},
           "sent": format!("sha256:{}", "ab".repeat(32))})
}

/// 一份请求：一句人话；`private` 有的，前面一段思考带上这一家的私有数据。
fn request(private: Option<&str>) -> Request {
    let mut messages = Vec::new();
    if let Some(family) = private {
        messages.push(Message::Assistant {
            blocks: vec![Block::Reasoning(Reasoning {
                text: "thought".to_string(),
                private: Some(miyu_kernel::block::Private {
                    driver: DriverFamily::parse(family).expect("家族"),
                    data: RawJson::of(&json!({"sig": family})).expect("JSON"),
                }),
            })],
        });
    }
    messages.push(Message::User {
        blocks: vec![Block::Text(Text {
            text: "hi".to_string(),
        })],
    });
    Request {
        tools: Vec::new(),
        system: "be kind".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

/// 照 `fake` 发、空闲时限 `idle` 的一次。
fn ready(fake: &Arc<Fake>, idle: Duration) -> Ready {
    let port: Arc<dyn DriverPort> = Arc::clone(fake) as Arc<dyn DriverPort>;
    Ready {
        client: miyu_http::client(Proxy::Off).expect("造得出客户端"),
        model: Model {
            endpoint: ProviderId::parse("ag").expect("编号"),
            model: ModelName::parse("m").expect("模型名"),
        },
        endpoint: Endpoint::keyless(""),
        speaker: Speaker::Extension(Box::new(Reached {
            driver: DriverRef {
                package: "pi".to_string(),
                name: "ag".to_string(),
            },
            port,
            reach: Reach {
                images: false,
                key: Some(Secret::new("sk-ag").expect("key")),
                base_url: None,
            },
            texts: hosted::texts(),
        })),
        call: Call {
            model: ModelName::parse("m").expect("模型名"),
            max_output: Some(100),
            inputs: Inputs::default(),
            effort: None,
            temperature: None,
        },
        idle,
        learn: Learn {
            data: Arc::new(ModelData::new(
                Profiles::default(),
                Vendors::default(),
                None,
            )),
            provider: "ag".to_string(),
            model: "m".to_string(),
            window: None,
        },
        tariff: None,
        placeholders: Vec::new(),
    }
}

/// 照剧本 `plans` 发一次 `request`：交回结尾、收到的增量、发没发出去。
async fn run(fake: Arc<Fake>, idle: Duration, request: Request) -> (Exchanged, Vec<Delta>, bool) {
    let ready = ready(&fake, idle);
    let Speaker::Extension(reached) = &ready.speaker else {
        unreachable!()
    };
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-ext-driver-unused"));
    let (mut deltas, mut sent) = (Vec::new(), false);
    let ended = exchange(
        &ready,
        reached,
        Cow::Owned(request),
        &blobs,
        std::future::pending(),
        |progress| match progress {
            Progress::Sent { .. } => sent = true,
            Progress::Delta(delta) => deltas.push(delta),
        },
    )
    .await;
    (ended, deltas, sent)
}

fn scripted(plans: Vec<Plan>) -> Arc<Fake> {
    Arc::new(Fake {
        plans: Mutex::new(plans.into()),
        ..Fake::default()
    })
}

/// 结尾的分类和原话。
fn error(ended: &Exchanged) -> Option<(ErrorClass, String)> {
    match ended {
        Exchanged::Ended {
            error: Some(classified),
            ..
        } => Some((
            classified.error.class.clone(),
            classified.error.message.clone(),
        )),
        _ => None,
    }
}

#[tokio::test]
async fn a_normal_call_streams_deltas_and_reports_usage_and_sent() {
    let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
    let (ended, deltas, sent) = run(Arc::clone(&fake), Duration::from_secs(5), request(None)).await;
    assert!(sent);
    assert_eq!(
        deltas,
        [
            Delta::Start {
                index: 0,
                kind: Kind::Text
            },
            Delta::Text {
                index: 0,
                text: "he".to_string()
            },
            Delta::Text {
                index: 0,
                text: "llo".to_string()
            },
            Delta::End { index: 0 },
        ]
    );
    let Exchanged::Ended { usage, error, wire } = ended else {
        panic!("说完了")
    };
    assert!(error.is_none());
    assert_eq!(usage.map(|usage| usage.uncached), Some(10));
    assert_eq!(
        wire.map(|hash| hash.to_string()),
        Some(format!("sha256:{}", "ab".repeat(32)))
    );
    assert!(
        fake.dropped.lock().expect("锁").is_empty(),
        "答完了的不发 driver.cancel"
    );
    let params = fake.params.lock().expect("锁")[0].clone();
    assert_eq!(params["driver"], "pi/ag");
    assert_eq!(params["key"], "sk-ag");
    assert!(params.get("base_url").is_none(), "没写地址的不交");
    assert!(params.get("blobs").is_none(), "没声明看图的不交图");
    assert_eq!(params["params"], json!({"model": "m", "max_output": 100}));
    assert_eq!(params["request"]["system"], "be kind", "规范字节原样");
    let hashes = &params["fingerprint"]["messages"];
    assert_eq!(hashes.as_array().map(Vec::len), Some(1));
    assert_eq!(hashes[0].as_str().map(str::len), Some(16), "十六进制的字");
}

#[tokio::test]
async fn broken_calls_end_with_their_class() {
    let half = vec![hello()[0].clone()];
    let disorder = vec![json!([{"text": {"index": 0, "text": "x"}}])];
    let garbled = json!({"usage": {"nope": 1}});
    for (plan, class, message) in [
        (
            Plan::Gone(half.clone()),
            ErrorClass::Retryable,
            "the driver went away",
        ),
        (
            Plan::Refuse,
            ErrorClass::Unclassified,
            "the driver refused the call: no such method",
        ),
        (
            Plan::Answer(disorder, Duration::ZERO, usage()),
            ErrorClass::Unclassified,
            "the driver sent a bad delta: ",
        ),
        (
            Plan::Answer(Vec::new(), Duration::ZERO, garbled),
            ErrorClass::Unclassified,
            "the driver sent a bad answer: ",
        ),
        (
            Plan::Answer(vec![json!([{"what": 1}])], Duration::ZERO, usage()),
            ErrorClass::Unclassified,
            "the driver sent a bad delta: ",
        ),
    ] {
        let fake = scripted(vec![plan]);
        let (ended, _, _) = run(Arc::clone(&fake), Duration::from_secs(5), request(None)).await;
        let (got, said) = error(&ended).expect("出错");
        assert_eq!(got, class, "{said}");
        assert!(said.starts_with(message), "{said}");
    }
}

#[tokio::test]
async fn a_bad_delta_drops_the_call() {
    let fake = scripted(vec![Plan::Hang(vec![
        json!([{"start": {"index": 1, "kind": "text"}}]),
    ])]);
    let (ended, deltas, _) = run(Arc::clone(&fake), Duration::from_secs(5), request(None)).await;
    assert!(error(&ended).is_some_and(|(class, _)| class == ErrorClass::Unclassified));
    assert!(deltas.is_empty(), "不对的那一条不交给内核");
    assert_eq!(
        fake.dropped.lock().expect("锁").len(),
        1,
        "不要了：端口发 driver.cancel"
    );
}

#[tokio::test]
async fn a_quiet_driver_times_out_but_a_trickling_one_does_not() {
    let fake = scripted(vec![Plan::Hang(vec![hello()[0].clone()])]);
    let (ended, deltas, _) =
        run(Arc::clone(&fake), Duration::from_millis(150), request(None)).await;
    assert_eq!(
        error(&ended),
        Some((ErrorClass::Stalled, "the driver went quiet".to_string())),
        "多久没动静的是卡住（施工 YC-3）"
    );
    assert_eq!(deltas.len(), 2, "超时以前交的照交");
    assert_eq!(
        fake.dropped.lock().expect("锁").len(),
        1,
        "超时的发 driver.cancel"
    );
    // 每段隔 60 毫秒、空闲时限 300 毫秒：十段一段一段地来，一共 600 毫秒，超过空闲时限也不超时（每来一段重新算）。
    let fake = scripted(vec![Plan::Answer(
        trickle(8),
        Duration::from_millis(60),
        usage(),
    )]);
    let (ended, deltas, _) = run(fake, Duration::from_millis(300), request(None)).await;
    assert_eq!(error(&ended), None);
    assert_eq!(deltas.len(), 10);
}

/// 一块正文：开始、`n` 段字、收尾，一份一条。
fn trickle(n: usize) -> Vec<Value> {
    let mut batches = vec![json!([{"start": {"index": 0, "kind": "text"}}])];
    batches.extend((0..n).map(|_| json!([{"text": {"index": 0, "text": "."}}])));
    batches.push(json!([{"end": {"index": 0}}]));
    batches
}

#[tokio::test]
async fn the_total_limit_holds_even_when_deltas_keep_coming() {
    // 空闲时限 100 毫秒、总时限 1 秒（十倍）：每 10 毫秒来一段，三百段要 3 秒，总时限先到，后面的不收。
    let fake = scripted(vec![Plan::Answer(
        trickle(300),
        Duration::from_millis(10),
        usage(),
    )]);
    let (ended, deltas, _) = run(fake, Duration::from_millis(100), request(None)).await;
    assert_eq!(
        error(&ended),
        Some((ErrorClass::Retryable, "the driver went quiet".to_string()))
    );
    assert!(deltas.len() < 302, "没等它交完：{}", deltas.len());
}

#[tokio::test]
async fn being_cancelled_drops_the_call() {
    let fake = scripted(vec![Plan::Hang(Vec::new())]);
    let ready = ready(&fake, Duration::from_secs(5));
    let Speaker::Extension(reached) = &ready.speaker else {
        unreachable!()
    };
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-ext-driver-unused"));
    let ended = exchange(
        &ready,
        reached,
        Cow::Owned(request(None)),
        &blobs,
        tokio::time::sleep(Duration::from_millis(50)),
        |_| {},
    )
    .await;
    assert!(matches!(ended, Exchanged::Cancelled));
    assert_eq!(
        fake.dropped.lock().expect("锁").len(),
        1,
        "叫停的发 driver.cancel"
    );
}

#[tokio::test]
async fn not_connected_yet_is_retryable_and_says_how_long() {
    let fake = Arc::new(Fake {
        not_yet: Some(NotYet {
            retry_in_ms: Some(1500),
        }),
        ..Fake::default()
    });
    let (ended, _, sent) = run(fake, Duration::from_secs(5), request(None)).await;
    assert!(!sent, "没发出去");
    let Exchanged::Ended {
        error: Some(classified),
        ..
    } = ended
    else {
        panic!("出错")
    };
    assert_eq!(classified.error.class, ErrorClass::Retryable);
    assert_eq!(classified.error.retry_after_ms, Some(1500));
}

#[tokio::test]
async fn only_this_drivers_private_data_is_handed_over() {
    for (family, kept) in [("pi/ag", true), ("anthropic", false), ("other/ag", false)] {
        let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
        let (ended, _, _) = run(
            Arc::clone(&fake),
            Duration::from_secs(5),
            request(Some(family)),
        )
        .await;
        assert_eq!(error(&ended), None);
        let params = fake.params.lock().expect("锁")[0].clone();
        let reasoning = &params["request"]["messages"][0]["blocks"][0];
        assert_eq!(reasoning["text"], "thought", "{family}：块还在");
        assert_eq!(reasoning.get("private").is_some(), kept, "{family}");
    }
}

#[tokio::test]
async fn private_deltas_are_owned_by_the_extension_family() {
    let batches = vec![json!([
        {"start": {"index": 0, "kind": "reasoning"}},
        {"private": {"index": 0, "data": {"sig": "s1"}}},
        {"end": {"index": 0}},
        "truncated"
    ])];
    let fake = scripted(vec![Plan::Answer(batches, Duration::ZERO, usage())]);
    let (_, deltas, _) = run(fake, Duration::from_secs(5), request(None)).await;
    let Some(Delta::Private { private, .. }) = deltas.get(1) else {
        panic!("{deltas:?}")
    };
    assert_eq!(private.driver.as_str(), "pi/ag");
    assert_eq!(private.data.get(), r#"{"sig":"s1"}"#);
    assert_eq!(deltas.last(), Some(&Delta::Truncated));
}
mod wired;

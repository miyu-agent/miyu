//! 金样（施工 V-38）：同一批请求经 [`exchange`] 发到本机的假服务器（请求体是片段清单，图片、PDF 的 base64 发的时候才从
//! blob 读出来编），服务器收到的字节和改前（`1b712ee5d`：整份编成字节，交给 reqwest 前再整份拷一份）一字不差。改前跑出来的
//! SHA-256 写死在 [`CASES`] 里：请求字节的、每条消息在字节里的起止的，还有发到哪条路径。「发出去了」报的哈希也得是它。
//!
//! 这一批里有：人附的带名字、带路径的图，PDF，文本文件（引号、反斜杠、控制字符、`</file>`、表情），工具结果里的文字夹着图
//! （拼字补换行），没有输出的出错结果，思考回传（各家自己的私有数据），工具调用的供应商编号，接着写，替它看的图，占位工具，
//! 思考强度、温度、输出上限。驱动：openai-chat 的 DeepSeek 那一套（回传思考、前缀续写、思考开关）和默认那一套（看不了图、
//! 读不了 PDF），anthropic，openai-responses。每一份照主请求的路（请求自己那一份交进去，编完就放）和旁路请求的路（借来的）
//! 各发一次，两回的字节一样。施工 V-42 起再照主请求留着编好的历史的路发两回（头一回从头编、留下；第二回整份接着用），四回的
//! 字节、哈希都一样。

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use miyu_drivers::openai_chat::{
    Compat, Continuation, ContinuationField, ReasoningField, ReasoningReplay, Toggle,
};
use miyu_drivers::{Anthropic, Call, Driver, EFFORT_ON, Inputs, Memo, OpenAiChat, OpenAiResponses};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Endpoint, Progress, Proxy, client};
use miyu_kernel::id::{ContentHash, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_store::blob::Blobs;

use super::super::base::{Ready, Speaker};
use super::super::shared::ModelData;
use super::samples::{Media, media, model, request, texts};
use super::{Learn, exchange};

/// 一份金样：叫什么、改前发出去的请求字节的哈希、每条消息起止的哈希、路径。
struct Case {
    name: &'static str,
    body: &'static str,
    ranges: &'static str,
    path: &'static str,
}

/// 改前（`1b712ee5d`）照这一批跑出来的。
const CASES: [Case; 4] = [
    Case {
        name: "deepseek",
        body: "sha256:c433efd9869b6f245ce1b3620368c04ca9f457bbd29ea75434c751971e9f3315",
        ranges: "sha256:f09c4d23a8b3a9d2de9931885696633f45f9c3dc089601dd628abc7201291208",
        path: "/v1/beta/chat/completions",
    },
    Case {
        name: "chat-default",
        body: "sha256:fbe33cf36d47ceeadd2431afe25503e7cca9619d51b9eb772b73a48b829b4acb",
        ranges: "sha256:8856f070fdb274365d692e35cd8bcaf9ff69ed55bcf8c7f7b8a0df40390a98ad",
        path: "/v1/chat/completions",
    },
    Case {
        name: "anthropic",
        body: "sha256:3fab8e0b730be32b1ed9f791e793169ef94b2a7b89b720fbf6e25e986c782d83",
        ranges: "sha256:d5cb6e35099b63d14af3d549bfe2deb675b06bb934e55c2d7fbaa5ba7294c463",
        path: "/v1/messages",
    },
    Case {
        name: "responses",
        body: "sha256:221fa7c367832eb5bf480da05d52df4484c61a3a765820b219bc8a2638757af1",
        ranges: "sha256:aa76bee7909b0fb9e8b943add6ed571359dc5ec8253a38426fd037e8be3fbf9b",
        path: "/v1/responses",
    },
];

/// 一份：驱动、调用、请求、占位工具。
pub(super) type Setup = (Box<dyn Driver>, Call, Request, Vec<(String, String)>);

/// 这一批的四份。
pub(super) fn cases(media: &Media) -> Vec<Setup> {
    let deepseek = Compat {
        reasoning: ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true,
        },
        continuation: Continuation::Prefix {
            field: ContinuationField::Prefix,
            path: "/beta/chat/completions".to_string(),
        },
        toggle: Some(Box::new(Toggle {
            field: "thinking".to_string(),
            on: serde_json::json!({"type": "enabled"}),
            off: serde_json::json!({"type": "disabled"}),
        })),
        ..Compat::default()
    };
    let chat = |family: &str| {
        request(
            media,
            family,
            r#"{"field":"reasoning_content"}"#,
            r#"{"id":"call_abc"}"#,
            true,
        )
    };
    let placeholders = vec![(
        "shell".to_string(),
        "Placeholder for a tool this client is expected to send with the request.".to_string(),
    )];
    vec![
        (
            Box::new(OpenAiChat::new(deepseek, texts())),
            Call {
                model: model("deepseek-v4"),
                max_output: Some(8192),
                inputs: Inputs {
                    images: true,
                    pdf: false,
                },
                effort: Some(EFFORT_ON.to_string()),
                temperature: Some(0.7),
            },
            chat("openai-chat"),
            placeholders.clone(),
        ),
        (
            Box::new(OpenAiChat::new(Compat::default(), texts())),
            Call {
                model: model("glm-5"),
                max_output: None,
                inputs: Inputs::default(),
                effort: None,
                temperature: None,
            },
            chat("openai-chat"),
            Vec::new(),
        ),
        (
            Box::new(Anthropic::new(texts())),
            Call {
                model: model("claude-opus-5"),
                max_output: Some(4096),
                inputs: Inputs {
                    images: true,
                    pdf: true,
                },
                effort: None,
                temperature: Some(1.0),
            },
            request(
                media,
                "anthropic",
                r#"{"signature":"sig-A"}"#,
                r#"{"id":"toolu_1"}"#,
                true,
            ),
            placeholders,
        ),
        (
            Box::new(OpenAiResponses::new(texts())),
            Call {
                model: model("gpt-5.4"),
                max_output: None,
                inputs: Inputs {
                    images: true,
                    pdf: true,
                },
                effort: None,
                temperature: None,
            },
            request(
                media,
                "openai-responses",
                r#"{"id":"rs_1","encrypted_content":"gAAArs_1"}"#,
                r#"{"id":"fc_1"}"#,
                false,
            ),
            Vec::new(),
        ),
    ]
}

/// 发给假服务器 `server` 的这一次。
pub(super) fn ready(
    server: &Server,
    driver: Box<dyn Driver>,
    call: Call,
    placeholders: Vec<(String, String)>,
) -> Ready {
    Ready {
        client: client(Proxy::Off).expect("造得出客户端"),
        model: Model {
            endpoint: ProviderId::parse("golden").expect("供应商名合写法"),
            model: call.model.clone(),
        },
        endpoint: Endpoint::new(server.base_url.clone(), "sk-golden"),
        speaker: Speaker::Builtin(driver),
        call,
        idle: Duration::from_secs(30),
        learn: Learn {
            data: Arc::new(ModelData::new(
                Profiles::default(),
                Vendors::default(),
                None,
            )),
            provider: "golden".to_string(),
            model: "golden".to_string(),
            window: None,
        },
        tariff: None,
        placeholders,
    }
}

/// 经 [`exchange`] 发一次，交回「发出去了」报的哈希。`memo` 有的照主请求留着编好的历史的路编（施工 V-42）。
async fn send(
    ready: &Ready,
    request: Cow<'_, Request>,
    memo: Option<&Mutex<Memo>>,
    blobs: &Blobs,
) -> Option<ContentHash> {
    let mut sent = None;
    exchange(
        ready,
        request,
        memo,
        blobs,
        std::future::pending(),
        |progress| {
            if let Progress::Sent { request, .. } = progress {
                sent = Some(request);
            }
        },
    )
    .await;
    sent
}

/// 每条消息在字节里的起止，照先后写成一串再算哈希。
fn ranges(ranges: &[std::ops::Range<usize>]) -> String {
    let text: Vec<String> = ranges
        .iter()
        .map(|range| format!("{}..{}", range.start, range.end))
        .collect();
    ContentHash::of(text.join(",").as_bytes()).to_string()
}

#[tokio::test]
async fn the_bytes_on_the_wire_are_the_same_as_before() {
    let dir = std::env::temp_dir().join(format!("miyu-v38-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了临时目录");
    // blob 只在数据根里建目录（施工 TMP-1）：写上标记。
    std::fs::write(
        dir.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得进标记");
    let blobs = Blobs::new(dir.join("blobs"));
    let media = media();
    for bytes in [&media.png, &media.jpeg, &media.pdf, &media.notes] {
        blobs.put(bytes).expect("存得进 blob");
    }
    let done = Reply::stream(vec![Piece::Bytes(b"data: [DONE]\n\n".to_vec())]);
    let mut seen = Vec::new();
    for ((driver, call, request, placeholders), case) in cases(&media).into_iter().zip(&CASES) {
        let server = Server::start(vec![done.clone(); 4]).await;
        let ready = ready(&server, driver, call, placeholders);
        // 主请求的路：自己那一份交进去；旁路请求、一次性的路：借来的。
        let owned = send(&ready, Cow::Owned(request.clone()), None, &blobs).await;
        let borrowed = send(&ready, Cow::Borrowed(&request), None, &blobs).await;
        // 主请求留着编好的历史（施工 V-42）：头一回从头编、留下，第二回整份接着用。
        let memo = Mutex::new(Memo::new());
        let cold = send(&ready, Cow::Owned(request.clone()), Some(&memo), &blobs).await;
        let warm = send(&ready, Cow::Owned(request.clone()), Some(&memo), &blobs).await;
        assert!(
            !memo.lock().expect("锁好的").is_empty(),
            "{}：留下了",
            case.name
        );
        let received = server.received();
        assert_eq!(received.len(), 4, "{}：四回都发出去了", case.name);
        for (index, other) in received.iter().enumerate().skip(1) {
            assert!(
                received[0].body == other.body,
                "{}：第 {} 回和第一回发出去的字节一样",
                case.name,
                index + 1
            );
            assert_eq!(received[0].path, other.path, "{}", case.name);
        }
        let body = &received[0].body;
        let hash = ContentHash::of(body);
        assert_eq!(
            owned.as_ref(),
            Some(&hash),
            "{}：报的哈希就是发出去的",
            case.name
        );
        assert_eq!(borrowed.as_ref(), Some(&hash), "{}", case.name);
        assert_eq!(cold.as_ref(), Some(&hash), "{}", case.name);
        assert_eq!(warm.as_ref(), Some(&hash), "{}", case.name);
        // 每条消息的起止照驱动直接编一回：和发出去的是同一串字节。
        let mut filled = request.clone();
        super::super::placeholder::fill(&mut filled, &ready.placeholders);
        let Speaker::Builtin(driver) = &ready.speaker else {
            unreachable!("样本都是内置驱动")
        };
        let needed = driver.blobs_needed(&filled, &ready.call);
        let fetched: BTreeMap<ContentHash, Vec<u8>> = needed
            .into_iter()
            .map(|hash| {
                let bytes = blobs.get(&hash).expect("取得出 blob");
                (hash, bytes)
            })
            .collect();
        let encoded = driver
            .encode(&filled, &ready.call, &fetched)
            .expect("编得了");
        assert!(
            encoded.body == *body,
            "{}：直接编的和发出去的一样",
            case.name
        );
        seen.push(format!(
            "{}: body {} ranges {} path {}",
            case.name,
            hash,
            ranges(&encoded.messages),
            received[0].path
        ));
        if hash.as_str() != case.body
            || ranges(&encoded.messages) != case.ranges
            || received[0].path != case.path
        {
            seen.push(format!(
                "  ^ 和金样不一样：{} {} {}",
                case.body, case.ranges, case.path
            ));
        }
    }
    miyu_store::testkit::remove_tree(&dir);
    assert!(
        !seen.iter().any(|line| line.starts_with("  ^")),
        "和改前的字节不一样：\n{}",
        seen.join("\n")
    );
}

//! 交给扩展驱动的请求里的供应商侧工具（施工 HT-2，设计 71 第三节第 5 条）：自己家带私有数据的原样留在规范字节里；别家的、
//! 没带私有数据的换成一块正文，内容和内置驱动写的那段字一样，位置不变；以前造的快照里没有那几份的拿掉；没有要改的请求原样
//! 交、不拷贝。

use std::borrow::Cow;

use miyu_drivers::{DriverTextSources, DriverTexts, HostedToolSources};
use miyu_kernel::block::{HostedTool, Outcome, Reasoning};

use super::*;

/// 驱动的占位：只有供应商侧工具那四份是出厂的，别的用不着；`hosted` 是假的照以前造的快照，没有那四份。
pub(crate) fn texts_with(hosted: bool) -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: "",
        file_omitted: "",
        no_output: "",
        tool_attachments: "",
        tool_attachments_only: "",
        text_file: None,
        image_name: None,
        image_description: None,
        attached_path: None,
        open_path: None,
        path_only: None,
        hosted_tool: hosted.then_some(HostedToolSources {
            hosted_tool: include_str!(
                "../../../../../../../resources/core/drivers/hosted-tool.txt"
            ),
            hosted_tool_failed: include_str!(
                "../../../../../../../resources/core/drivers/hosted-tool-failed.txt"
            ),
            hosted_tool_open: include_str!(
                "../../../../../../../resources/core/drivers/hosted-tool-open.txt"
            ),
            hosted_tool_cut: include_str!(
                "../../../../../../../resources/core/drivers/hosted-tool-cut.txt"
            ),
        }),
    })
    .expect("占位用得了")
}

/// 一块供应商侧工具：`driver` 有的带上那个家族的私有数据。
pub(crate) fn hosted(call: &str, driver: Option<&str>) -> Block {
    Block::HostedTool(HostedTool {
        call_id: CallId::parse(call).expect("编号"),
        name: "web_search".to_string(),
        args: r#"{"query":"<rust>"}"#.to_string(),
        outcome: Some(Outcome {
            text: "found & done".to_string(),
            failed: false,
            cut: false,
        }),
        private: driver.map(|driver| Private {
            driver: DriverFamily::parse(driver).expect("家族"),
            data: RawJson::of(&json!({"id": "srvtoolu_1"})).expect("JSON"),
        }),
    })
}

fn said(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn request(blocks: Vec<Block>) -> Request {
    Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![
            Message::User {
                blocks: vec![said("搜")],
            },
            Message::Assistant { blocks },
        ],
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

fn assistant(request: &Request) -> &[Block] {
    let Message::Assistant { blocks } = &request.messages[1] else {
        panic!("第二条是助手消息");
    };
    blocks
}

/// 内置驱动写的那段字。
fn written(block: &Block) -> String {
    let Block::HostedTool(hosted) = block else {
        panic!("供应商侧工具");
    };
    texts_with(true).hosted_tool(hosted).expect("出厂的带着")
}

#[test]
fn its_own_family_s_hosted_tool_is_handed_over_as_it_is() {
    let request = request(vec![
        said("a"),
        hosted("call_1_1", Some("pi/ag")),
        said("b"),
    ]);
    assert!(!foreign(&request, &family()));
    let handed = handed(Cow::Borrowed(&request), &family(), &texts_with(true));
    assert!(matches!(handed, Cow::Borrowed(_)), "没有要改的不拷贝");
    let bytes = String::from_utf8(handed.canonical_bytes()).expect("UTF-8");
    assert!(bytes.contains(r#""type":"hosted_tool""#), "{bytes}");
    assert!(bytes.contains("srvtoolu_1"), "私有数据原样：{bytes}");
}

#[test]
fn others_and_bare_ones_become_the_same_text_in_place() {
    for driver in [None, Some("anthropic"), Some("other/ag")] {
        let block = hosted("call_1_1", driver);
        let request = request(vec![said("a"), block.clone(), said("b")]);
        assert!(foreign(&request, &family()), "{driver:?}");
        let handed = handed(Cow::Borrowed(&request), &family(), &texts_with(true));
        assert!(matches!(handed, Cow::Owned(_)), "{driver:?}");
        assert_eq!(
            assistant(&handed),
            [said("a"), said(&written(&block)), said("b")],
            "{driver:?}"
        );
        let bytes = String::from_utf8(handed.canonical_bytes()).expect("UTF-8");
        assert!(!bytes.contains("hosted_tool"), "{bytes}");
        assert!(!bytes.contains("srvtoolu_1"), "别家的私有数据不交：{bytes}");
    }
}

/// 自己家的、别家的混在一起：各照各的，先后不乱；别家的思考照旧只拿掉私有数据。
#[test]
fn a_mixed_reply_keeps_its_order() {
    let theirs = hosted("call_1_2", Some("anthropic"));
    let thought = Block::Reasoning(Reasoning {
        text: "t".to_string(),
        private: Some(Private {
            driver: DriverFamily::parse("anthropic").expect("家族"),
            data: RawJson::of(&json!("sig")).expect("JSON"),
        }),
    });
    let request = request(vec![
        thought,
        hosted("call_1_1", Some("pi/ag")),
        theirs.clone(),
        said("done"),
    ]);
    let handed = handed(Cow::Borrowed(&request), &family(), &texts_with(true));
    let blocks = assistant(&handed);
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[0], Block::Reasoning(r) if r.private.is_none()));
    assert_eq!(blocks[1], hosted("call_1_1", Some("pi/ag")));
    assert_eq!(blocks[2], said(&written(&theirs)));
    assert_eq!(blocks[3], said("done"));
}

/// 以前造的快照里没有那几份：别家的、没带私有数据的拿掉（不能原样交，又没有登记过的字可写），自己家的照留。
#[test]
fn an_older_snapshot_drops_the_others() {
    let request = request(vec![
        said("a"),
        hosted("call_1_1", Some("anthropic")),
        hosted("call_1_2", None),
        hosted("call_1_3", Some("pi/ag")),
        said("b"),
    ]);
    let handed = handed(Cow::Borrowed(&request), &family(), &texts_with(false));
    assert_eq!(
        assistant(&handed),
        [said("a"), hosted("call_1_3", Some("pi/ag")), said("b")]
    );
}

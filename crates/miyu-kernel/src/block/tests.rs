//! 内容块的测试：图纸上的几种读写一字不差；供应商侧工具（施工 HT-1）；驱动私有数据、工具名和参数、不认识的块，
//! 都一字不差；附件原来的路径（施工 3-9 五补）；只有路径的附件（施工 AT-1）；坏的报错。

use super::*;
use crate::test_support::{rejected, round_trip};

const HASH: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn every_block_from_the_drawing_round_trips() {
    for json in [
        r#"{"type":"text","text":"我先看一下目录。"}"#.to_string(),
        r#"{"type":"reasoning","text":"先看目录","private":{"driver":"anthropic","data":{"signature":"sig"}}}"#.to_string(),
        r#"{"type":"reasoning","text":"没有私有数据"}"#.to_string(),
        format!(r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","width":800,"height":600}}"#),
        format!(r#"{{"type":"image","blob":"{HASH}","name":"晚霞.png","media_type":"image/png","width":800,"height":600}}"#),
        format!(r#"{{"type":"file","blob":"{HASH}","name":"报告.pdf","media_type":"application/pdf"}}"#),
        r#"{"type":"tool_call","call_id":"call_44_1","name":"read","args":"{\"path\":\"src\"}"}"#.to_string(),
    ] {
        round_trip::<Block>(&json);
        let block: Block = serde_json::from_str(&json).unwrap();
        assert!(!matches!(block, Block::Unknown(_)), "{json} 应该认得出种类");
    }
}

/// 驱动私有数据里的空格、数字的写法（1.50）都原样留着。
#[test]
fn private_data_is_kept_byte_for_byte() {
    round_trip::<Block>(
        r#"{"type":"reasoning","text":"…","private":{"driver":"anthropic","data":{ "signature" : "EqQBCkYI", "n": 1.50, "list": [ ] }}}"#,
    );
}

#[test]
fn tool_call_keeps_name_and_args_as_the_model_wrote_them() {
    round_trip::<Block>(
        r#"{"type":"tool_call","call_id":"call_44_2","name":"不存在的 工具!","args":"{ \"path\" : \"src\" , }"}"#,
    );
    round_trip::<Block>(
        r#"{"type":"tool_call","call_id":"call_44_3","name":"read","args":"{}","private":{"driver":"openai-chat","data":"call_abc123"}}"#,
    );
}

/// 图片块的名字（施工 3-9 四补）：人附的带着，`read` 读出来的、以前的日志里的没有，读回来是没有，写出去也没有这一格。
#[test]
fn an_image_carries_its_name_only_when_it_has_one() {
    let named = format!(
        r#"{{"type":"image","blob":"{HASH}","name":"晚霞.png","media_type":"image/png","width":800,"height":600}}"#
    );
    let Block::Image(image) = serde_json::from_str(&named).unwrap() else {
        panic!("是图片块");
    };
    assert_eq!(image.name.as_ref().map(FileName::as_str), Some("晚霞.png"));
    let old = format!(
        r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","width":800,"height":600}}"#
    );
    let Block::Image(image) = serde_json::from_str(&old).unwrap() else {
        panic!("是图片块");
    };
    assert_eq!(image.name, None);
    assert!(
        !serde_json::to_string(&Block::Image(image))
            .unwrap()
            .contains("name")
    );
}

/// 附件原来的路径（施工 3-9 五补）：图片块、文件块带着的读写一字不差，排在最后；没有的读成没有，写出去也没有这一格。
#[test]
fn an_attachment_carries_where_it_came_from_only_when_it_has_one() {
    for text in [
        format!(
            r#"{{"type":"image","blob":"{HASH}","name":"晚霞.png","media_type":"image/png","width":800,"height":600,"path":"/home/a/晚霞.png"}}"#
        ),
        format!(
            r#"{{"type":"file","blob":"{HASH}","name":"报告.pdf","media_type":"application/pdf","path":"~/报告.pdf"}}"#
        ),
    ] {
        round_trip::<Block>(&text);
    }
    let old = format!(
        r#"{{"type":"file","blob":"{HASH}","name":"报告.pdf","media_type":"application/pdf"}}"#
    );
    let Block::File(file) = serde_json::from_str(&old).unwrap() else {
        panic!("是文件块");
    };
    assert_eq!(file.path, None);
    assert_eq!(serde_json::to_string(&Block::File(file)).unwrap(), old);
}

/// 只有路径的附件（施工 AT-1）：认得出种类，读写一字不差；没写大小的读成没有，写出去也没有这一格。名字、路径照
/// 各自的写法查。
#[test]
fn a_path_only_attachment_is_known_and_round_trips() {
    let sized = r#"{"type":"path","name":"大.iso","path":"/data/大.iso","size":31457280}"#;
    let bare = r#"{"type":"path","name":"notes.md","path":"~/notes.md"}"#;
    for json in [sized, bare] {
        round_trip::<Block>(json);
        let block: Block = serde_json::from_str(json).unwrap();
        assert!(!matches!(block, Block::Unknown(_)), "{json} 应该认得出种类");
    }
    rejected::<Block>(
        r#"{"type":"path","name":"a/b.iso","path":"/data/b.iso"}"#,
        "bad file name",
    );
    rejected::<Block>(
        r#"{"type":"path","name":"b.iso","path":"data/b.iso"}"#,
        "bad source path",
    );
    rejected::<Block>(
        r#"{"type":"path","name":"b.iso","path":"/data/b.iso","size":-1}"#,
        "expected u64",
    );
    rejected::<Block>(r#"{"type":"path","name":"b.iso"}"#, "path");
}

/// 供应商侧工具（施工 HT-1）：认得出种类，带不带结果、私有数据，`failed`、`cut` 是真的写、是假的不写，读写一字不差；
/// 结果里缺了 `failed`、`cut` 的读成假；缺格的报错。
#[test]
fn a_hosted_tool_round_trips_with_or_without_outcome() {
    for json in [
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{\"query\":\"rust\"}"}"#,
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"text":"1. Rust"}}"#,
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"text":"","failed":true}}"#,
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"text":"很长","cut":true}}"#,
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"text":"x","failed":true,"cut":true},"private":{"driver":"anthropic","data":{ "id" : "srvtoolu_1" }}}"#,
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"WebSearch","args":"","private":{"driver":"openai-chat","data":"ws_1"}}"#,
    ] {
        round_trip::<Block>(json);
        let block: Block = serde_json::from_str(json).unwrap();
        assert!(
            matches!(block, Block::HostedTool(_)),
            "{json} 应该认得出种类"
        );
    }
    let Block::HostedTool(hosted) = serde_json::from_str(
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"text":"ok"}}"#,
    )
    .unwrap() else {
        panic!("是供应商侧工具");
    };
    assert_eq!(
        hosted.outcome,
        Some(Outcome {
            text: "ok".into(),
            failed: false,
            cut: false
        })
    );
    // 它不是正文：一串块里的字不算它。
    assert_eq!(words(&[Block::HostedTool(hosted)]), "");
    rejected::<Block>(
        r#"{"type":"hosted_tool","call_id":"call_44_2","args":"{}"}"#,
        "name",
    );
    rejected::<Block>(
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search"}"#,
        "args",
    );
    rejected::<Block>(
        r#"{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{}","outcome":{"failed":true}}"#,
        "text",
    );
    rejected::<Block>(
        r#"{"type":"hosted_tool","call_id":"h1","name":"web_search","args":"{}"}"#,
        "bad call id",
    );
}

#[test]
fn unknown_block_is_kept_byte_for_byte() {
    let json = format!(r#"{{"type":"audio", "blob":"{HASH}","seconds":3.20}}"#);
    let block: Block = serde_json::from_str(&json).unwrap();
    assert!(matches!(block, Block::Unknown(_)), "{block:?}");
    assert_eq!(serde_json::to_string(&block).unwrap(), json);
}

#[test]
fn broken_blocks_are_errors() {
    rejected::<Block>(
        &format!(r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","height":600}}"#),
        "width",
    );
    rejected::<Block>(r#"{"text":"没有 type"}"#, "missing field `type`");
    rejected::<Block>(
        &format!(
            r#"{{"type":"image","blob":"{HASH}","name":"shots/a.png","media_type":"image/png","width":1,"height":1}}"#
        ),
        "bad file name",
    );
    rejected::<Block>(
        r#"{"type":"file","blob":"sha256:12","name":"a.txt","media_type":"text/plain"}"#,
        "bad content hash",
    );
    rejected::<Block>(
        r#"{"type":"tool_call","call_id":"c1","name":"read","args":"{}"}"#,
        "bad call id",
    );
    rejected::<Block>(
        r#"{"type":"reasoning","text":"x","private":{"driver":"Anthropic","data":{}}}"#,
        "bad driver family",
    );
}

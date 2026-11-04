//! 主请求照会话那样留着编好的历史编（施工 V-42），和整份编的比。

use std::collections::BTreeMap;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{Anthropic, Call, Driver, Inputs, Memo, OpenAiChat, OpenAiResponses};
use miyu_kernel::id::{ModelName, Seq};
use miyu_kernel::request::Request;

use super::driver_texts;

/// 主请求一次次照会话那样留着编好的历史编（施工 V-42，[`Memo`]）：三家（DeepSeek 那一套、anthropic、openai-responses）各一份，
/// 每一次和整份编的片段清单、每条消息的位置、路径一模一样。整份编的和存档比过，所以留着编的也和存档一个字节不差。
pub fn kept_matches(requests: &[(Seq, Request)]) {
    let drivers: [(Box<dyn Driver>, Call); 3] = [
        (
            Box::new(OpenAiChat::new(Compat::deepseek(), driver_texts())),
            call("deepseek-v4", Some(8192)),
        ),
        (
            Box::new(Anthropic::new(driver_texts())),
            call("claude-opus-5", Some(8192)),
        ),
        (
            Box::new(OpenAiResponses::new(driver_texts())),
            call("gpt-5.4", None),
        ),
    ];
    let blobs = BTreeMap::new();
    for (driver, call) in &drivers {
        let mut memo = Memo::new();
        for (index, (_, request)) in requests.iter().enumerate() {
            let whole = driver.fragments(request, call, &blobs);
            let kept = driver.fragments_memo(request, call, &blobs, &mut memo);
            assert!(
                whole == kept,
                "{} 第 {} 次请求：留着编的和整份编的不一样",
                driver.family(),
                index + 1
            );
        }
    }
}

/// 发给 `model` 的一次调用：看不了图、读不了 PDF，没有思考强度、温度。
fn call(model: &str, max_output: Option<u32>) -> Call {
    Call {
        model: ModelName::parse(model).expect("模型名合写法"),
        max_output,
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    }
}

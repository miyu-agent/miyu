//! 守卫拒绝时写给模型的那一句、给人看的说法，要问人时给头看的说明（施工 HK-1，设计 65 第三节「拒绝的理由」）。
//!
//! 理由是第三方写的字，会进模型的上下文：照不可信的字段处理。去掉头尾的空白、截到 [`MOST_CHARS`] 个字，换进出厂的模板
//! `core/guard/blocked.txt`（`Blocked by {package}: {reason}`）：换的时候照模板转义（`08-上下文投影.md` 第五节，
//! [`miyu_kernel::template::escape`]），控制字符、换行、引号、尖括号都写成 `\n`、`\u003c` 这样，整句一行，伪造不出另一条记录、另一个
//! 说话的人、一段工具结果。包编号也一样转义。没答、答不了的那几句是核心自己的字（`core/guard/*.txt`），同样换进这个模板。
//! 几份字编进程序（同 `miyu-cli` 的 `core/call/`）：不用读盘，拒绝的这一路没有读不出来的时候。字都登记在 `26-提示词.md`
//! 第十节。
//!
//! 给人看的说法（`said`）带的是截过的原文，不转义：头照自己的规矩转义（第三节「拒绝的理由」第 4 条）。

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde_json::json;

use miyu_kernel::event::Said;
use miyu_kernel::raw::RawJson;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Worded;

use super::answer::Answer;

/// 拒绝的那一句的模板：字段 `package`、`reason`。
const BLOCKED: &str = include_str!("../../../../../resources/core/guard/blocked.txt");
/// 拒绝了、没写理由的。
const NO_REASON: &str = include_str!("../../../../../resources/core/guard/no-reason.txt");
/// 没在时限里答的（「守卫超时」）。
const TIMED_OUT: &str = include_str!("../../../../../resources/core/guard/timed-out.txt");
/// 开着却没在跑的、连接断了的（「守卫不在」）。
const NOT_RUNNING: &str = include_str!("../../../../../resources/core/guard/not-running.txt");
/// 回了 JSON-RPC 错误的（「守卫出错」）。
const FAILED: &str = include_str!("../../../../../resources/core/guard/failed.txt");
/// 答得不对的（「守卫答得不对」）。
const MALFORMED: &str = include_str!("../../../../../resources/core/guard/malformed.txt");

/// 理由至多几个字（设计 65 第十节第 2 条）。
pub(super) const MOST_CHARS: usize = 300;

/// 读好的模板：编进程序的，测试里试换过（`tests.rs`）。
static TEMPLATE: LazyLock<Template> =
    LazyLock::new(|| Template::parse(BLOCKED).expect("编进来的模板测试里读过"));

/// 包 `package` 照 `answer` 拒绝了（拒绝的、没答的、答不了的）：写给模型的一句和给人看的说法。
pub(super) fn denied(package: &str, answer: &Answer) -> Worded {
    let (reason, said) = match answer {
        Answer::Deny(Some(reason)) if !cut(reason).is_empty() => {
            let reason = cut(reason);
            let said = Said::new("core/guard/denied-with-reason").with("reason", reason.clone());
            (reason, said)
        }
        Answer::Deny(_) => (line(NO_REASON), Said::new("core/guard/denied")),
        Answer::TimedOut => (line(TIMED_OUT), Said::new("core/guard/timed-out")),
        Answer::Absent => (line(NOT_RUNNING), Said::new("core/guard/not-running")),
        Answer::Failed => (line(FAILED), Said::new("core/guard/failed")),
        // 放行、问人的不会走到这里；万一走到了照答得不对说，照样拒绝。
        Answer::Malformed | Answer::Allow | Answer::Ask(_) => {
            (line(MALFORMED), Said::new("core/guard/malformed"))
        }
    };
    Worded {
        text: render(package, &reason),
        said: Some(said.with("package", package)),
    }
}

/// 包 `package` 要问人：给头看的说明，哪件工具、谁要问、它写的理由（截过，没写的不写这一格）。
pub(super) fn asking(package: &str, tool: &str, reason: Option<&str>) -> Option<RawJson> {
    let mut detail = json!({"guard": package, "tool": tool});
    if let Some(reason) = reason.map(cut).filter(|reason| !reason.is_empty()) {
        detail["reason"] = json!(reason);
    }
    RawJson::of(&detail)
}

/// 去掉头尾的空白，截到 [`MOST_CHARS`] 个字。
pub(super) fn cut(reason: &str) -> String {
    reason.trim().chars().take(MOST_CHARS).collect()
}

/// 出厂的一句理由：去掉行尾的换行（它是换进模板的字段，不是整句）。
fn line(text: &str) -> String {
    text.trim_end_matches(['\n', '\r']).to_string()
}

/// 换进模板：字段照模板转义。
///
/// # Panics
///
/// 实际不会：编进来的模板只要 `package`、`reason` 两个字段，测试里试换过。
fn render(package: &str, reason: &str) -> String {
    let fields = BTreeMap::from([("package", package), ("reason", reason)]);
    TEMPLATE
        .render(&fields)
        .expect("编进来的模板测试里试换过，字段都有")
}

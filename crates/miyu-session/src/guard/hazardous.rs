//! 认出了高危的怎么判（施工 5-17、HZ-1，D-1 从 `guard.rs` 挪出来）：灭顶的拒，别的问人、不提放行规则。报路径的调用问人、
//! 提放行规则的那一路随私密目录去掉了（施工 PV-RM）。

use serde_json::{Value, json};

use miyu_kernel::session::Verdict;
use miyu_kernel::tool::Access;

use super::{module, raw};

/// 认出了高危的（施工 5-17）：原来就拒的照拒；灭顶的拒，写给她工具交的那一句；别的问人，不提规则（高危的不该一劳永逸），说明
/// 照不报路径的那种（第五条第 4 款）再多 `hazard`。问人的那几种里，归 `waived` 里哪一类的不问（施工 HZ-1，设置页「受保护的
/// 命令」关掉的那几类）：还有要问的照最先认出的那一种问；都不用问的交回空的，照没认出高危接着判。
pub(super) fn hazardous(
    verdict: &Verdict,
    (name, access): (&str, Access),
    hazard: miyu_tool::Hazard,
    waived: &[&str],
    asking: &[(&'static str, String)],
) -> Option<Verdict> {
    if matches!(verdict, Verdict::Deny { .. }) {
        return Some(verdict.clone());
    }
    let asks = match hazard {
        miyu_tool::Hazard::Refused(worded) => return Some(super::deny(worded)),
        miyu_tool::Hazard::Asks(asks) => asks,
    };
    let standing = asks
        .iter()
        .find(|ask| ask.class.is_none_or(|class| !waived.contains(&class)))?;
    let mut detail: Value = serde_json::from_str(super::described(name, &access, asking).get())
        .unwrap_or_else(|_| json!({ "tool": name }));
    detail["hazard"] = json!(standing.code);
    Some(Verdict::Ask {
        module: module(),
        access,
        rule: None,
        detail: Some(raw(&detail)),
    })
}

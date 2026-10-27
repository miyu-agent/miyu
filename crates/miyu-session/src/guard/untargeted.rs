//! 不报路径的调用怎么判（从 `guard.rs` 挪来，施工 5-18：那边到了 500 行）：照访问类别、级别、沙盒能不能用，在场所里做的事
//! 照会话在不在场所里，要在沙盒外跑的照级别（`session/guard.md` 第四条）。

use std::path::Path;

use serde_json::json;

use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::Access;
use miyu_policy::GuardTexts;

use super::asking::{prefixed, this_dir};
use super::levels::Effective;
use super::{Place, deny, module, raw};

/// 不报路径的调用：执行命令，沙盒能用（`sandboxed`）就工作区、只读都放行，在沙盒里跑；用不了的问人（施工 5-4 上）。
/// 读写放行（查不到路径的执行时自己报错），联网、对外发消息这些还没有的，除了完全放开都问人。问人时的说明并进工具交的
/// `asking`（施工 D-4）。
pub(super) fn untargeted(
    level: Effective,
    name: &str,
    access: Access,
    sandboxed: bool,
    asking: &[(&'static str, String)],
) -> Verdict {
    let fine = matches!(
        (&access, level),
        (_, Effective::Full) | (Access::Read | Access::Write, _)
    ) || (access == Access::Execute && sandboxed);
    if fine {
        return Verdict::Allow;
    }
    let detail = described(name, &access, asking);
    Verdict::Ask {
        module: module(),
        access,
        rule: None,
        detail: Some(detail),
    }
}

/// 在场所里做的事（访问类别 `venue`，施工 O-31 前，第四条）：场所会话里放行、不问人；本机的会话拒绝。
pub(super) fn in_venue(place: &Place) -> Verdict {
    match place {
        Place::Venue => Verdict::Allow,
        Place::Local(worded) => deny(worded.clone()),
    }
}

/// 要在沙盒外跑的（施工 D-4，第四条）：拒绝的照拒（数据根这些），完全放开照判的（本来就不套沙盒），只读拒绝，工作区问人。
/// 执行命令的（`access` 是执行）问人时另提「此目录不再询问」：放行工作目录 `cwd`（真实的位置）的写（施工 5-15），工具报了命令
/// 前缀 `prefix` 的带上它（施工 SB-3）；本会话放行过（`grants`）同一件工具、同一前缀、这个目录（连同下面的）的，工作区这一级
/// 不再问。高危、只碰得到自己工作区的在这之前拦下了。
pub(super) fn beyond(
    verdict: Verdict,
    level: Effective,
    (name, access, prefix, cwd): (&str, Access, Option<String>, &Path),
    (grants, asking): (&[RawJson], &[(&'static str, String)]),
    texts: &GuardTexts,
) -> Verdict {
    match (verdict, level) {
        (denied @ Verdict::Deny { .. }, _) => denied,
        (verdict, Effective::Full) => verdict,
        (_, Effective::ReadOnly) => deny(texts.read_only()),
        (_, Effective::Workspace)
            if prefix
                .as_deref()
                .is_some_and(|prefix| prefixed(grants, name, prefix, cwd)) =>
        {
            Verdict::Allow
        }
        (_, Effective::Workspace) => {
            let rule = (access == Access::Execute).then(|| this_dir(name, cwd, prefix.as_deref()));
            let detail = described(name, &access, asking);
            Verdict::Ask {
                module: module(),
                access,
                rule,
                detail: Some(detail),
            }
        }
    }
}

/// 不报路径的调用问人时的说明（第五条第 4 款，施工 D-4）：工具交的几格，再写上工具名；执行类的写明这一次不在沙盒里跑
/// （走到问人的执行，不是要在沙盒外跑，就是沙盒用不了）。键照字母排。
pub(super) fn described(name: &str, access: &Access, asking: &[(&'static str, String)]) -> RawJson {
    let mut detail = json!({});
    for (key, value) in asking {
        detail[*key] = json!(value);
    }
    detail["tool"] = json!(name);
    if *access == Access::Execute {
        detail["sandbox"] = json!(false);
    }
    raw(&detail)
}

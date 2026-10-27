//! 软件后台（施工 F-6 中，`docs/blueprint/package-pages.md`，设计 `30-插件框架.md` 第十三节）：带自己页面的软件，页面的文件
//! 经 `package.file` 拿（头不读核心的存储，远程的头也照样能用），页面要它的程序做的事经 `package.call` 转过去：程序先用
//! `package.methods` 登记方法，核心反向发 `method.call`。页面能做的只有这几样，别的本事由它的程序用自己批过的能力做。

mod calls;
mod events;
mod file;

pub(crate) use calls::{Methods, call, register};
pub(crate) use events::{emit, mine};
pub(crate) use file::{beneath, read, segments};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 成员碰不碰得到包 `package` 的后台页（施工 IM-1 中下）：只碰得到每人一份、他能用的（勾了「成员也能用」），那一份是他自己的；
/// 整台机器一份的后台页只给管理员。管理员、扩展（照 `packages.call` 能力挡）不查。不能的 `not_permitted`。
pub(crate) fn opened_to(core: &Core, caller: &Caller, package: &str) -> Result<(), Refusal> {
    if caller.package.is_some() || crate::login::admins::is_admin(core, caller.acting_for()) {
        return Ok(());
    }
    members_open(core, package)
        .then_some(())
        .ok_or(Refusal::NOT_PERMITTED)
}

/// 包 `package` 的后台页成员打不打得开：每人一份、他能用（勾了「成员也能用」）的才打得开。挡调用（[`opened_to`]）和给成员的
/// 软件包列表留不留 `page`（`packages.rs`）照同一个判法。
pub(crate) fn members_open(core: &Core, package: &str) -> bool {
    core.system_packages()
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
        .is_some_and(|manifest| {
            crate::extensions::per_account(manifest) && core.members_may(package, manifest)
        })
}

//! 供应商自己的联网工具（设计 `72-DeepSeek内置联网工具.md` 第二节，施工 DSR-3）：回合开始照模型端口这一刻说的判断要不要
//! 换掉 Miyu 的搜索工具，换掉的几件从工具面里拿掉，不换了的补回来。判断本身在 `miyu_policy::provider_search`，结果钉在
//! 策略快照里：会话中途只改「联网服务」的不变，换了模型、模型这边满不满足条件变了才重新判断。

use miyu_config::{Value, Values};
use miyu_policy::provider_search::strip;
use miyu_policy::{SearchOffer, SearchPin, Snapshot, ToolEntry};
use miyu_tool::Catalog;

use crate::port::ModelPort;

/// 模型端口这一刻说的：会话的模型引用，和模型这边给不给得了（供应商、官方地址、驱动、思考没关都对得上才给得了）。
pub(crate) struct SearchNow {
    /// 会话这时生效的引用；不知道的（测试的端口）是空的。
    pub(super) model: String,
    /// 给得了的那一份；给不了的没有。
    pub(super) offer: Option<SearchOffer>,
}

impl SearchNow {
    /// 问端口 `model`。回合开始时端口已经照这一轮的配置解析过引用。
    pub(crate) fn of(model: &dyn ModelPort) -> SearchNow {
        SearchNow {
            model: model.reference().unwrap_or_default(),
            offer: model.search_offer(),
        }
    }
}

/// 照这一刻判断要钉的。`fresh` 是照现在的目录、预设筛出来的整份工具面；没有的（预设找不回来的会话）、`now` 没有的
/// （只干算的那几处不问端口）照旧快照里钉的。
pub(super) fn judged(
    now: Option<&SearchNow>,
    old: &Snapshot,
    values: &Values,
    fresh: Option<&[ToolEntry]>,
) -> Option<SearchPin> {
    let (Some(now), Some(fresh)) = (now, fresh) else {
        return old.provider_search.clone();
    };
    let picked = match values.get(miyu_config::package::SEARCH_SOURCE) {
        Some(Value::Text(picked)) => picked.as_ref(),
        _ => "",
    };
    SearchPin::judged(
        old.provider_search.as_ref(),
        &now.model,
        now.offer.as_ref(),
        picked,
        fresh,
    )
}

/// 工具面 `face` 照新钉的 `new` 换：旧快照换掉、现在该在的那几件照 `fresh` 补回来（别处重筛工具面只照旧快照里有的对，
/// 不会自己加回来），再把现在换掉的拿掉；跑命令的工具说明末尾「脚本里能调哪几件」的名单跟着重写
/// （[`crate::agents::clicall::redescribe`]）。`fresh` 没有的照原样。
pub(super) fn swapped(
    face: Vec<ToolEntry>,
    fresh: Option<&[ToolEntry]>,
    old: &Snapshot,
    new: Option<&SearchPin>,
    catalog: &Catalog,
) -> Vec<ToolEntry> {
    let Some(fresh) = fresh else {
        return face;
    };
    let was = old
        .provider_search
        .iter()
        .flat_map(|pin| pin.declared.iter());
    let back: Vec<ToolEntry> = was
        .filter(|replaced| !face.iter().any(|entry| entry.name == replaced.tool))
        .filter_map(|replaced| fresh.iter().find(|entry| entry.name == replaced.tool))
        .cloned()
        .collect();
    // 说明是照补回来以前、拿掉以前的那一份名单写的：补回来的那几件照 `fresh` 拿，本身不跑命令。
    let before = face.clone();
    let mut face = strip(face.into_iter().chain(back).collect(), new);
    crate::agents::clicall::redescribe(catalog, &before, &mut face);
    face
}

/// 快照 `snapshot` 钉着的、发请求时要声明的供应商侧工具名。
pub(super) fn names(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .provider_search
        .as_ref()
        .map(SearchPin::names)
        .unwrap_or_default()
}

//! 估一条事件多大用的几样：图片、文件的价钱（施工 6-3 上），照这个会话的组装器写成的块估（施工 CT-2，[`Weigh`]），本地估算
//! 和真值的倍数（施工 CT-2，[`Scale`]）。

use crate::estimate::{self, Price, Scale, Weigh, WithImages};
use crate::history::History;
use crate::session::Session;

impl Session {
    /// 估算图片、文件的办法：驱动交了图片算法的照它，别的照策略里的固定数。策略里没有压缩、没交限额的，没有。
    pub(in crate::session) fn price(&self) -> Option<WithImages<'_>> {
        Some(WithImages {
            images: self.limits.as_ref()?.images.as_deref(),
            flat: self.policy.compaction.as_ref()?.price,
        })
    }

    /// 这个会话现在的模型上，本地估算和真值的倍数（[`estimate::scale`]）；没交限额的是一倍。
    pub(in crate::session) fn scale(&self) -> Scale {
        self.limits.as_ref().map_or(Scale::ONE, |limits| {
            estimate::scale(&self.history, &limits.model)
        })
    }

    /// 照有效历史 `history` 估一条事件进请求以后多大（[`Weigh`]）：照这个会话的组装器写成的块数，图片、文件照 `price`。
    pub(in crate::session) fn weigh<'a>(
        &'a self,
        history: &'a History,
        price: &'a dyn Price,
    ) -> Weigh<'a> {
        Weigh {
            assembler: self.policy.assembler.as_ref(),
            history,
            price,
        }
    }
}

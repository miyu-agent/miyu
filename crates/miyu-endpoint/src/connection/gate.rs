//! 凭据决定的闸（施工 W-8）：用一次性码连上的只能设访问密码，别的方法 `setup_first`。主人什么都调得了，核心拉起的扩展
//! 照它的能力挡（`rights.rs`）。

use crate::login::Via;
use crate::refusal::Refusal;

/// 用凭据 `via` 连上的连接能不能调 `method`：不能的交回拒绝。
pub(super) fn refuse(via: Option<&Via>, method: &str) -> Option<Refusal> {
    match via {
        Some(Via::Code) => (!matches!(method, "hello" | "human.get" | "account.setup"))
            .then_some(Refusal::SETUP_FIRST),
        _ => None,
    }
}

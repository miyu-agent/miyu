//! DeepSeek 官方的余额（施工 QU-2）：`GET https://api.deepseek.com/user/balance`，回
//! `{"is_available": bool, "balance_infos": [{"currency", "total_balance", "granted_balance", "topped_up_balance"}…]}`
//! （官方文档 <https://api-docs.deepseek.com/api/get-user-balance>）。金额是字符串，照原样交，不换成数（头照原样画，省得
//! 小数点后面的位数变了）；币种照官方写的，不认死 CNY、USD 两种。多出来的格不管；缺格、类型不对的整份当没有。

use serde::Deserialize;
use serde_json::Value;

/// 余额。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    /// 能不能用（官方的 `is_available`：余额够不够调用）。
    pub available: bool,
    /// 每种币一条，照官方的先后。
    pub items: Vec<BalanceItem>,
}

/// 一种币的余额，金额照官方原样的字符串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceItem {
    /// 币种（官方写 `CNY`、`USD`）。
    pub currency: String,
    /// 一共多少（赠送的加充值的）。
    pub total: String,
    /// 赠送的。
    pub granted: String,
    /// 充值的。
    pub topped_up: String,
}

/// 官方的写法。
#[derive(Deserialize)]
struct Reply {
    is_available: bool,
    balance_infos: Vec<Info>,
}

#[derive(Deserialize)]
struct Info {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

/// 读 DeepSeek 的回应。
///
/// # Errors
///
/// 缺 `is_available`、`balance_infos`，或者哪一条缺格、金额不是字符串：英文的一句。
pub(super) fn deepseek(value: Value) -> Result<Balance, String> {
    let reply: Reply =
        serde_json::from_value(value).map_err(|error| format!("not a balance: {error}"))?;
    Ok(Balance {
        available: reply.is_available,
        items: reply
            .balance_infos
            .into_iter()
            .map(|info| BalanceItem {
                currency: info.currency,
                total: info.total_balance,
                granted: info.granted_balance,
                topped_up: info.topped_up_balance,
            })
            .collect(),
    })
}

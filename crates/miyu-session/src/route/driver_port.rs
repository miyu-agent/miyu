//! 扩展提供的模型驱动的通道，执行器这一头看到的样子（施工 MD-2，设计 66 第三、五节）：扩展在上一层的 `miyu-endpoint` 里，执行器
//! 只认这个端口。核心起来以后装进模型资料（[`crate::ModelData::on_drivers`]，照「下架」那个端口的做法）；没装的（测试、第一次接入）
//! 扩展驱动照旧当用不了。
//!
//! 端口只管两样：这个驱动这一刻在不在、声明了什么（[`DriverPort::present`]），和把一次调用送过去、把增量和回答送回来
//! （[`DriverPort::call`]）。参数怎么编、增量怎么查、时限、出错归类都在执行器里（`route/extension.rs`）。

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use tokio::sync::mpsc;

use miyu_config::package::DriverKey;
use miyu_models::provider::DriverRef;

/// 装着、批了、开着的驱动在清单里声明的（设计 66 第四节第 1 条）：执行器照它定交不交 key、交不交图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    /// 要不要核心交这一家的 key（第九节第 3 条）。
    pub key: DriverKey,
    /// 看得了图：请求里的图照原样交过去（第五节第 1 条）。
    pub images: bool,
}

/// 驱动在，这一刻却连不上（设计 66 第八节第 5 条）：正在退避、等着重新拉起，或者刚拉起、还没握手。执行器当可重试。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYet {
    /// 退避中的，还要等多久重新拉起（`extension.status` 的 `retry_in`）；刚拉起的没有。
    pub retry_in_ms: Option<u64>,
}

/// 送过去了，没等到回答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Broken {
    /// 连接断了：扩展崩了、退出了、被停下了。
    Gone,
    /// 扩展回了 JSON-RPC 的错误（不是回答里的 `error`）：原话。
    Refused(String),
}

/// 送出去的一次调用，等它的回答：`driver.call` 的 `result` 原样。丢掉它（叫停、超时、增量不对）就是不要了：端口发
/// `driver.cancel`，之后来的这一次的增量都丢掉。
pub type Pending = Pin<Box<dyn Future<Output = Result<Value, Broken>> + Send>>;

/// 扩展驱动的通道。
pub trait DriverPort: Send + Sync + std::fmt::Debug {
    /// 驱动 `driver` 这一刻在不在：在的交它的声明；不在的（没装、清单里没有这个驱动名、包关着、没批 `model.driver`、停下了）交一句
    /// 英文的原因，执行器当这家供应商用不了、不重试（设计 66 第八节第 5 条）。
    ///
    /// # Errors
    ///
    /// 不在：原因。
    fn present(&self, driver: &DriverRef) -> Result<Declared, String>;

    /// 往驱动 `driver` 所在的扩展发一次 `driver.call`：`call` 是这一次的编号（增量照它对），`params` 是写好的参数，一个 JSON 对象的
    /// 原文（里面已经有 `call`），原样放进那一行、不再解析（1 MB 的请求不多走一遍）。这一次的每一条 `driver.delta` 照到的先后把它的
    /// `deltas` 原样交进 `deltas`；回答之前到的增量一定已经交了（同一条连接，先后是保住的）。
    ///
    /// # Errors
    ///
    /// 驱动在，这一刻却连不上（[`NotYet`]）：没发出去。
    fn call(
        &self,
        driver: &DriverRef,
        call: &str,
        params: String,
        deltas: mpsc::UnboundedSender<Value>,
    ) -> Result<Pending, NotYet>;

    /// 往驱动 `driver` 所在的扩展发一条只要回答、不交增量的请求 `method`（施工 MD-4：`driver.models`），`params` 同上是写好的原文。
    /// 丢掉交回的 [`Pending`] 就是不等了，不另发通知。不实现的端口（测试的假端口）当连不上。
    ///
    /// # Errors
    ///
    /// 驱动在，这一刻却连不上（[`NotYet`]）：没发出去。
    fn ask(&self, driver: &DriverRef, method: &str, params: String) -> Result<Pending, NotYet> {
        let _ = (driver, method, params);
        Err(NotYet { retry_in_ms: None })
    }
}

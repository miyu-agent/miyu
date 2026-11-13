//! 不经核心、照配置直接连的那一个模型（施工 RC-1，`docs/blueprint/cli/rescue.md`）：`miyu rescue` 拉起 LiteMiyu 的应急模式时，
//! 照 Miyu 的 `models.chat` 交给它地址、模型名、驱动和 key。核心可能正坏着，所以这里只照这一刻的配置和资料算，不问核心。
//!
//! - 模型 `p/m`：照 [`crate::provider::provider`] 合出那一家，再照真发的那个模型换驱动（[`Provider::for_model`]），和核心发请求时
//!   一样。
//! - 池 `@名字`：照写的先后取第一个用得了的、走内置驱动的成员。用得了：那一家合得出、这个模型有驱动、地址解得出也不是空的、写了
//!   key 的取得到 key。走扩展驱动的成员跳过（LiteMiyu 认不了，core1 2026-11-13 定）；一个内置的都用不了时，交第一个扩展驱动的成员，
//!   由调的一方照「扩展驱动」说。冷却、指针、钉着的都不看：那些记在核心的状态里，应急时不读。
//! - 走扩展驱动的（`<包>/<驱动名>`）照实交出驱动，地址可能是空的、key 不取：调的一方认了驱动就不用它们。
//!
//! 这一层不碰文件和环境，密钥、环境变量里的地址照调的一方交进来的 `secret` 取。

use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference as KeyRef, Secret};

use crate::facts::facts;
use crate::knowledge::Knowledge;
use crate::provider::{self, Driver, NOT_CONFIGURED, NoModel, Provider, key_name};
use crate::reference::{self, Resolved as Points};

/// 照配置直接连的一个模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Direct {
    /// 地址，路径由驱动接在后面：内置驱动的不是空的；扩展驱动的可能是空的（施工 MD-2：扩展可以不要地址）。
    pub base_url: String,
    /// 模型名，照供应商那边的叫法。
    pub model: String,
    /// 驱动：照真发的这个模型（同一家里的模型可以各走各的，施工 8-14）。扩展提供的也照实交，调的一方照它说用不了。
    pub driver: Driver,
    /// key：那一家没写的（本机的服务）、扩展驱动的没有。
    pub key: Option<Secret>,
}

/// 照最终的配置 `resolved`、手头的资料 `knowledge` 算 `models.chat` 直接连的模型；密钥、环境变量里的地址照 `secret` 取。
///
/// # Errors
///
/// `models.chat` 没配；引用解析不出（[`reference::resolve`]）；指到的模型用不了（地址、key 取不到，驱动还没有）；池里内置的、扩展的
/// 用得了的成员都没有（原话带上最后一个成员为什么用不了）。原话都是英文，照 `models.md`「出错」那张表的口气。
pub fn direct(
    resolved: &Resolved,
    knowledge: &Knowledge<'_>,
    secret: &dyn Fn(&KeyRef) -> Option<Secret>,
) -> Result<Direct, NoModel> {
    let values = resolved.values();
    let chat = provider::chat(&values).ok_or_else(|| NoModel(NOT_CONFIGURED.to_string()))?;
    match reference::resolve(&values, knowledge, &chat)? {
        Points::Model(target) => {
            reach(resolved, knowledge, &target.provider, &target.model, secret)
        }
        Points::Pool(pool) => {
            let mut why = String::new();
            let mut extension = None;
            for member in &pool.members {
                let reached = provider::provider(&values, knowledge, &member.provider)
                    .and_then(|one| reach(resolved, knowledge, &one, &member.model, secret));
                match reached {
                    Ok(direct) if direct.driver.extension().is_some() => {
                        extension.get_or_insert(direct);
                    }
                    Ok(direct) => return Ok(direct),
                    Err(NoModel(said)) => why = said,
                }
            }
            if let Some(direct) = extension {
                return Ok(direct);
            }
            Err(NoModel(format!(
                "no usable model in pool {:?}: {why}",
                pool.name
            )))
        }
    }
}

/// 一家 `provider` 的模型 `model` 能不能直接连：换成真发时的驱动，解出地址，取 key。
fn reach(
    resolved: &Resolved,
    knowledge: &Knowledge<'_>,
    provider: &Provider,
    model: &str,
    secret: &dyn Fn(&KeyRef) -> Option<Secret>,
) -> Result<Direct, NoModel> {
    let (facts, _) = facts(resolved, knowledge, provider, model);
    let speaking = provider.for_model(model, &facts.wire, &knowledge.profiles.npm)?;
    // 扩展驱动的不往下取：调的一方认了驱动就不用地址和 key，取不到也不算用不了。
    if speaking.driver.extension().is_some() {
        return Ok(Direct {
            base_url: provider::resolve_base_url(&speaking, secret).unwrap_or_default(),
            model: model.to_string(),
            driver: speaking.driver,
            key: None,
        });
    }
    let base_url = provider::resolve_base_url(&speaking, secret)?;
    if base_url.is_empty() {
        return Err(NoModel(format!(
            "provider {:?} has no base_url",
            speaking.id
        )));
    }
    let key = match &speaking.key {
        None => None,
        Some(reference) => Some(secret(reference).ok_or_else(|| {
            NoModel(format!(
                "key {} of provider {:?} is not set",
                key_name(reference),
                speaking.id
            ))
        })?),
    };
    Ok(Direct {
        base_url,
        model: model.to_string(),
        driver: speaking.driver,
        key,
    })
}

#[cfg(test)]
mod tests;

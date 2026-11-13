//! `miyu rescue` 照 Miyu 的对话模型配 LiteMiyu 的应急模式（施工 RC-1，`docs/blueprint/cli/rescue.md`）：核心可能正坏着，所以
//! 不连核心、不拉起核心，在这个进程里照磁盘读一遍，同 CPU-1 在建运行时以前读系统配置（`workers.rs`）。
//!
//! 读的：系统配置（只认模型这一块的几项）、密钥文件、资源目录里的供应商档案和认原厂的表、models.dev 的目录（快照和缓存挑新的，
//! 不整理、不写盘：程序位置不交，[`crate::models::catalog`] 就不存整理好的一份）。用出来的、供应商的列表、池的指针、冷却都不读：
//! 记在核心的状态里，应急时不靠它们。挑哪一个模型由 [`miyu_models::direct`] 算。
//!
//! 什么都不写：数据根不建骨架、不拿锁、不装运行日志。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_config::problem::Problem;
use miyu_config::secret::{Reference, Secret, Stored, parse_file};
use miyu_config::{Item, Layer};
use miyu_models::Knowledge;
use miyu_models::catalog::CatalogLookup;
use miyu_models::direct::{Direct, direct};
use miyu_models::observed::Learned;
use miyu_models::provider::NoModel;
use miyu_models::settings::{
    FIXED_POOL_ITEMS, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::models::{self, catalog::Places};

/// 照这个进程的环境算 [`model_in`]。
///
/// # Errors
///
/// 同 [`model_in`]。
pub fn model() -> Result<Direct, String> {
    model_in(&Env::current())
}

/// 照环境 `env` 找数据根、资源目录，算 `models.chat` 直接连的模型。密钥照密钥文件，`{ env = … }` 照这个进程的环境变量。
///
/// # Errors
///
/// 数据根、资源目录找不到；系统配置、密钥文件读不了、整份写坏了；档案、认原厂的表读不出来；模型用不了（原话照
/// [`miyu_models::direct::direct`]）。都交回英文的原因。
pub fn model_in(env: &Env) -> Result<Direct, String> {
    let root = DataRoot::locate(env).map_err(|error| error.to_string())?;
    let resources = ResourceRoot::locate(env).map_err(|error| error.to_string())?;
    let resolved = config(&root.config_file())?;
    let stored = secrets(&root.secrets_file())?;
    let profiles = resources
        .profiles()
        .map_err(|error| error.to_string())
        .and_then(|text| models::profiles(&text))?;
    let vendors = resources
        .vendors()
        .map_err(|error| error.to_string())
        .and_then(|text| models::vendors(&text))?;
    let catalog = models::catalog::load(&Places {
        snapshot: resources
            .catalog_snapshot()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        cache: models::cache(env),
        program: None,
    });
    let (learned, lists) = (Learned::default(), BTreeMap::new());
    let knowledge = Knowledge {
        profiles: &profiles,
        vendors: &vendors,
        catalog: catalog.as_ref(),
        lookup: catalog.as_ref().map(CatalogLookup::Full),
        learned: &learned,
        lists: &lists,
    };
    let secret = |reference: &Reference| match reference {
        Reference::Secret(name) => stored.entries.get(name).cloned(),
        Reference::Env(name) => std::env::var(name)
            .ok()
            .and_then(|value| Secret::new(&value).ok()),
    };
    direct(&resolved, &knowledge, &secret).map_err(|NoModel(why)| why)
}

/// 模型这一块的配置项：用途、池（连三个固定池）、供应商、模型手写的资料。别的项写在配置里照不认识的丢掉，不碍事。
fn items() -> Vec<Item> {
    [
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        FIXED_POOL_ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
    ]
    .concat()
}

/// 读系统配置 `path`、合出最终值：没有这个文件的照什么都没写。写错的一项照核心的规矩丢掉、别的照用（G8）；环境变量照这个进程的。
fn config(path: &Path) -> Result<Resolved, String> {
    let text = miyu_store::config_file::read(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .map(|read| read.text)
        .unwrap_or_default();
    let items = items();
    let parsed = parse(&items, Layer::System, &text)
        .map_err(|problem| format!("{}: {}", path.display(), told(&problem)))?;
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    Ok(merge(&items, &layers, &|name| std::env::var(name).ok()))
}

/// 读密钥文件 `path`：没有这个文件的是空的；写错的一行照核心的规矩不收、别的照收。
fn secrets(path: &Path) -> Result<Stored, String> {
    let Some(read) =
        miyu_store::secrets::read(path).map_err(|error| format!("{}: {error}", path.display()))?
    else {
        return Ok(Stored::default());
    };
    parse_file(&read.text.text).map_err(|problem| format!("{}: {}", path.display(), told(&problem)))
}

/// 整份读不懂的那一条说成一句：第几行、为什么（`toml_edit` 的原话最后一行，不带原文：密钥文件的原文就是密钥）。
fn told(problem: &Problem) -> String {
    let why = problem
        .why
        .clone()
        .unwrap_or_else(|| problem.code.as_str().to_string());
    match problem.at {
        Some(at) => format!("line {}: {why}", at.line),
        None => why,
    }
}

#[cfg(test)]
mod tests;

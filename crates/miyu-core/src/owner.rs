//! 主人（施工 SU-1，项目主人 2026-10-28 定「移除所有的多用户设计」）：Miyu 只给一个人用。起来时照 `system/accounts.json` 找主人
//! （[`owner`]）；还没有的（第一次起来）当场建一个（[`ensure`]）：名字取系统的用户名，取不成的叫 `me`；不设访问密码（要从局域网
//! 连时再用 `account.setup` 设）。没有「只能设置」的那一段了。

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::accounts;
use miyu_store::root::DataRoot;

use crate::TARGET;

/// 取不到系统用户名时主人叫什么。
const FALLBACK: &str = "me";

/// 主人：账号文件里标了 `owner` 的那一个。没有这个文件、读不了、还没建的没有（读不了的记一行）。
pub fn owner(root: &DataRoot) -> Option<AccountId> {
    let path = root.path().join(accounts::FILE);
    let read = accounts::read(&path)
        .inspect_err(|error| tracing::warn!(target: TARGET, error = %error, "accounts not read"))
        .ok()?;
    let owner = read.accounts.owner()?;
    AccountId::parse(&owner.id)
        .inspect_err(|_| tracing::warn!(target: TARGET, "owner id not usable"))
        .ok()
}

/// 找主人，没有的当场建一个、记进账号文件：名字照 `user`（系统的用户名，照账号的写法换成小写、去掉不认的字），换不成的叫
/// `me`。
///
/// # Errors
///
/// 账号文件写不下。
pub fn ensure(root: &DataRoot, user: Option<&str>, now: Timestamp) -> Result<AccountId, String> {
    if let Some(owner) = owner(root) {
        return Ok(owner);
    }
    let account = user
        .map(name_of)
        .and_then(|name| AccountId::parse(&name).ok())
        .or_else(|| AccountId::parse(FALLBACK).ok())
        .ok_or_else(|| "no usable owner name".to_string())?;
    let path = root.path().join(accounts::FILE);
    std::fs::create_dir_all(root.path()).map_err(|error| error.to_string())?;
    let read = accounts::read(&path).ok();
    let mut written = read
        .as_ref()
        .map(|read| read.accounts.clone())
        .unwrap_or_default();
    written.put(accounts::Account {
        id: account.as_str().to_string(),
        username: account.as_str().to_string(),
        password: String::new(),
        changed: now,
        created: Some(now),
        disabled: false,
        owner: true,
    });
    accounts::write(
        &path,
        &written,
        read.and_then(|read| read.version).as_deref(),
    )
    .map_err(|error| error.to_string())?;
    tracing::info!(target: TARGET, "owner created");
    Ok(account)
}

/// 系统用户名换成账号的写法：小写，只留小写字母、数字、`-`、`_`，开头不是字母的前面补 `u`。
fn name_of(user: &str) -> String {
    let kept: String = user
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        .collect();
    match kept.chars().next() {
        Some(first) if first.is_ascii_lowercase() => kept,
        Some(_) => format!("u{kept}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;

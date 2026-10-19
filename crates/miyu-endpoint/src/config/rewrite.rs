//! 照一段代码改一层配置文件的字（施工 X-26，`mcp.set`、`mcp.remove`）：配置清单装不下的表（个人设置的 `[mcp.<名字>]`，
//! `docs/blueprint/mcp.md`「个人设置里的一张表」第 2 条）也照 `config.set` 的规矩写（`set.rs` 第 3 到 5 步）：
//!
//! 1. 拿着配置服务的锁，先把这一层重读一遍：手改过的在新的字上改，那一瞬间之前的手改先当手改推、记。
//! 2. 文件读不进来的不改：`config_file_broken`。还没有这份文件的，从新建的样子（第一行指向 Schema）改起。
//! 3. 先写临时文件再替换，替换前发现有人手改了，从第 1 步重来，最多三次，还不行的回 `config_conflict`。
//! 4. 写成了：换上新的一份、记日志、推 `config.changed`（没有配置项变的，`keys` 是空的）。监视着配置的几处（个人设置里的
//!    MCP 服务，`mcp/home.rs`）照新的一份跟着换。

use miyu_config::Layer;
use miyu_config::edit;
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, ConfigText, WriteError};

use super::observe::observe;
use super::push::Via;
use super::set::{TRIES, broken, done};
use super::{Config, TARGET};
use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 改不成的两种。
pub(crate) enum Unwritten {
    /// 放不进去：这份字的写法放不下这一处（同 `config.set` 放不进去的一项），回 `config_file_broken`。
    Blocked,
    /// 照这个原因拒。
    Refused(Refusal),
}

impl From<Refusal> for Unwritten {
    fn from(refusal: Refusal) -> Unwritten {
        Unwritten::Refused(refusal)
    }
}

/// 照 `edit` 改 `layer` 这一层的整份字：`edit` 拿到这一刻的字，交回改好的；什么都不用改的交回空的，不写。`cause` 是这条
/// 命令的编号，记进日志；改的人是这个连接的账号。每重来一次 `edit` 照新读的字再调一次。
///
/// # Errors
///
/// `edit` 拒的照它拒；放不进去、文件读不进来的 `config_file_broken`；三次都撞上手改的 `config_conflict`；写不成的
/// `internal_error`。
pub(crate) fn rewrite(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    layer: Layer,
    mut edit: impl FnMut(&str) -> Result<Option<String>, Unwritten>,
) -> Result<(), Refusal> {
    let words = words(core, peer.language)?;
    let mut config = core.config();
    let by = By::Person(Person::new(config.places.account.clone()));
    for _ in 0..TRIES {
        observe(core, &mut config, layer);
        let file = config.file(layer);
        if file.broken.is_some() {
            return Err(broken(&config, file, &words)?);
        }
        let start = match file.version {
            Some(_) => file.text.clone(),
            None => edit::new_file(Config::schema(layer)),
        };
        let text = match edit(&start) {
            Ok(Some(text)) if text != start => text,
            Ok(_) => return Ok(()),
            Err(Unwritten::Blocked) => return Err(broken(&config, file, &words)?),
            Err(Unwritten::Refused(refusal)) => return Err(refusal),
        };
        let bytes = config_file::bytes(&text, file.bom);
        match config_file::write(&file.path, &bytes, file.version.as_deref()) {
            Ok(()) => {
                let written = ConfigText {
                    version: config_file::version(&bytes),
                    text,
                    bom: file.bom,
                };
                done(
                    core,
                    &mut config,
                    layer,
                    written,
                    (Via::Set, by),
                    Some(cause),
                );
                return Ok(());
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Io(error)) => {
                tracing::warn!(target: TARGET, file = %file.shown, error = %error, "config not written");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    observe(core, &mut config, layer);
    Err(Refusal::config_conflict_version(
        config.file(layer).version.clone(),
    ))
}

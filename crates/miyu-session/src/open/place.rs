//! 会话在哪：本机的还是场所的；外部身份的会话关起来。

use miyu_kernel::id::VenueId;
use miyu_store::resources::{ResourceRoot, SourceError};

use crate::guard::{Confined, Place};

/// 会话在哪（施工 O-31 前，`session/guard.md` 第四条）：场所是 `local` 的是本机的会话，读出厂的那一句拒绝在场所里做的事；别的
/// 是场所会话。`confined` 的关起来（外部身份的会话：群、陌生人的私聊）：只碰得到自己的工作区（施工 5-12）。造会话、载入时各读
/// 一次，不进策略快照（[`ResourceRoot::not_in_venue`]、[`ResourceRoot::confined_texts`]）。
pub(super) fn place(
    venue: &VenueId,
    resources: &ResourceRoot,
    confined: bool,
) -> Result<(Place, Option<Confined>), SourceError> {
    let local = venue.as_str() == crate::agents::LOCAL;
    let place = match local {
        true => Place::local(resources.not_in_venue()?),
        false => Place::Venue,
    };
    if !confined {
        return Ok((place, None));
    }
    let (outside, no_commands) = resources.confined_texts()?;
    let confined = Confined::new(&outside, no_commands).map_err(|error| SourceError::Read {
        path: resources
            .path()
            .join("core")
            .join("permissions")
            .join("outside-workspace.txt"),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()),
    })?;
    Ok((place, Some(confined)))
}

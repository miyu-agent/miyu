//! 会话在哪：本机的还是场所的，场所的属主不是管理员的关进工作区。

use miyu_kernel::id::VenueId;
use miyu_store::resources::{ResourceRoot, SourceError};

use crate::guard::{Confined, Place};

/// 会话在哪（施工 O-31 前，`session/guard.md` 第四条）：场所是 `local` 的是本机的会话，读出厂的那一句拒绝在场所里做的事；别的
/// 是场所会话，属主不是管理员的只碰得到自己的工作区（施工 5-12，[`Confined`]）。造会话、载入时各读一次，不进策略快照
/// （[`ResourceRoot::not_in_venue`]、[`ResourceRoot::confined_texts`]）。
pub(super) fn place(
    venue: &VenueId,
    resources: &ResourceRoot,
    owner_is_admin: bool,
) -> Result<(Place, Option<Confined>), SourceError> {
    if venue.as_str() == crate::agents::LOCAL {
        return Ok((Place::local(resources.not_in_venue()?), None));
    }
    if owner_is_admin {
        return Ok((Place::Venue, None));
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
    Ok((Place::Venue, Some(confined)))
}

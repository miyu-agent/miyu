//! 会话在哪：本机的还是场所的；属主不是管理员的关起来。

use miyu_kernel::id::VenueId;
use miyu_store::resources::{ResourceRoot, SourceError};

use crate::guard::{Confined, Own, Place};

/// 会话在哪（施工 O-31 前，`session/guard.md` 第四条）：场所是 `local` 的是本机的会话，读出厂的那一句拒绝在场所里做的事；别的
/// 是场所会话。属主不是管理员的关起来（`own` 是属主自己的几处，管理员的没有）：场所会话只碰得到自己的工作区（施工 5-12），
/// 本机的会话（成员的，施工 Y-2 下①）只到 `own` 那几处（[`Confined::only`]）。造会话、载入时各读一次，不进策略快照
/// （[`ResourceRoot::not_in_venue`]、[`ResourceRoot::confined_texts`]）。
pub(super) fn place(
    venue: &VenueId,
    resources: &ResourceRoot,
    own: Option<Own>,
) -> Result<(Place, Option<Confined>), SourceError> {
    let local = venue.as_str() == crate::agents::LOCAL;
    let place = match local {
        true => Place::local(resources.not_in_venue()?),
        false => Place::Venue,
    };
    let Some(own) = own else {
        return Ok((place, None));
    };
    let (outside, no_commands) = resources.confined_texts()?;
    let confined = Confined::new(&outside, no_commands).map_err(|error| SourceError::Read {
        path: resources
            .path()
            .join("core")
            .join("permissions")
            .join("outside-workspace.txt"),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()),
    })?;
    Ok((
        place,
        Some(if local { confined.only(own) } else { confined }),
    ))
}

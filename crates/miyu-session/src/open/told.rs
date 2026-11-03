//! 测试用（施工 V-39 下的对照测试，`crate::testkit::told`）：照载入的读法读一个会话、交给内核载入，读到的、载入出来的写成字。
//! 从 `load.rs` 挪出来：那一份到了行数上限。

use miyu_kernel::facts::Environment;
use miyu_kernel::session::Session;
use miyu_store::blob::Blobs;

/// 照载入的读法读会话 `id`、交给内核载入，读到的、载入出来的都写成字（施工 V-39 下的对照测试，[`crate::testkit::Told`]）：
/// 分叉读好交过来的底子 `forked` 照载入一样用，用没用上照样比。不起 actor、不接记忆；载入的时刻、环境是定的。
pub(crate) fn told(
    root: &miyu_store::root::DataRoot,
    id: &miyu_kernel::id::SessionId,
    forked: Option<crate::ForkedBase>,
) -> crate::testkit::Told {
    let dir = root.session_dir(id);
    let blobs = Blobs::new(root.blobs());
    let read = match super::load::stream::read(&dir, id, &blobs, root, forked) {
        Ok(read) => read,
        Err(error) => {
            return crate::testkit::Told {
                loaded: format!("Err({error:?})"),
                primed: None,
                whole: false,
            };
        }
    };
    let super::load::stream::Read {
        log,
        created,
        base,
        snapshot: (_, policy),
        loaded,
        scan,
        primed,
        snapshotted: _,
    } = read;
    drop(log);
    let at = miyu_kernel::time::Timestamp::from_unix_millis(1_793_000_000_000)
        .unwrap_or(miyu_kernel::time::Timestamp::LATEST);
    let environment = Environment {
        offset: miyu_kernel::time::UtcOffset::UTC,
        cwd: "/w".to_string(),
        dirs: Vec::new(),
    };
    let whole = matches!(loaded, super::load::stream::Loaded::Whole { .. });
    let session = match loaded {
        super::load::stream::Loaded::Fed(loader) => loader.finish(at, policy, environment),
        super::load::stream::Loaded::Whole { base, events } => match base {
            Some(base) => Session::load_forked(id.clone(), &base, events, at, policy, environment),
            None => Session::load(id.clone(), events, at, policy, environment),
        },
    };
    crate::testkit::Told {
        loaded: format!("{created:?}\n{base:?}\n{scan:?}\n{session:?}"),
        primed: primed.map(|primed| format!("{primed:?}")),
        whole,
    }
}

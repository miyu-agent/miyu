//! 载入时找技能（施工 X-10 上、X-11 上；2026-11-18 从 `load.rs` 挪出来，那边过了 500 行）：这一刻找到的技能里，会话的预设开着的
//! 那几个的文件夹放开只读，连同她叫得出的名字一起交回。列给她看的那一块照快照，不在这里。

use std::path::{Path, PathBuf};

use miyu_config::Values;
use miyu_kernel::id::VenueId;
use miyu_store::root::DataRoot;

use crate::blocking::blocking;
use crate::open::{Installed, PresetPlaces};
use crate::skills;

/// 会话钉的预设 `wanted`（照几层 `places` 找文件，照场所 `venue`、显示不显示图片 `images` 定），配置 `values` 关没关、别的目录
/// 在哪（`home`），在阻塞线程里找。没交装了的软件的（测试里自己造的）什么都不放开。
pub(super) async fn opened(
    installed: Option<Installed>,
    root: &DataRoot,
    (wanted, places): (Option<String>, Option<PresetPlaces>),
    (values, home): (&Values, Option<&Path>),
    (images, venue): (bool, &VenueId),
) -> (Vec<PathBuf>, skills::Names) {
    let Some(installed) = installed else {
        return (Vec::new(), skills::Names::new());
    };
    let root = root.clone();
    let off = places
        .as_ref()
        .map(PresetPlaces::switched_off)
        .unwrap_or_default();
    let looked = (crate::switches::of(values), off);
    // 别的目录照这时的配置（施工 PT-2 外）：关着的不碰。
    let external = miyu_store::skills::external::of(values, home);
    let site = venue.clone();
    blocking(move || {
        let file = wanted
            .zip(places)
            .and_then(|(id, places)| places.file(&id, (images, site.as_str()), looked));
        let found = skills::find(&installed(), &root, &external);
        let found = skills::opened(found, file.as_ref());
        (skills::shelves(&found), skills::names(&found))
    })
    .await
}

//! 人格图片的版本和候选主色（施工 V-16）：第一次算、写进 `state/personas/pictures.json`；同一张图（位置、大小、修改时刻对得上）
//! 照记下的给、不读图，换了个进程也一样；换了图重算；先只要版本、后来要候选主色的补算。

use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};

use super::*;
use crate::test_support::temp_root;

/// 一张真的 PNG：左边 `left` 列是红的，右边是蓝的。
fn picture(left: u32) -> Vec<u8> {
    let image = RgbaImage::from_fn(64, 64, |x, _| {
        Rgba(if x < left {
            [200, 30, 40, 255]
        } else {
            [30, 60, 210, 255]
        })
    });
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("写得成 PNG");
    bytes.into_inner()
}

fn seeds_of(bytes: &[u8]) -> Option<Vec<String>> {
    miyu_seeds::from_image(bytes)
        .map(|found| found.into_iter().map(miyu_seeds::Seed::hex).collect())
}

#[test]
fn a_picture_is_looked_at_once_and_remembered_on_disk() {
    let (_root, dir) = temp_root("glance-once");
    let state = dir.join("state");
    let path = dir.join("avatar.png");
    let red = picture(48);
    std::fs::write(&path, &red).expect("写得进");

    let first = look(&state, &path, true).expect("看得到");
    assert_eq!(first.version, ContentHash::of(&red).hex()[..16]);
    assert_eq!(first.seeds, seeds_of(&red));
    assert!(state.join(FILE).is_file(), "记进了 state");

    // 换一个数据根再换回来：这一份从盘上重读，像核心重启了。
    let other = dir.join("other");
    look(&other, &path, false).expect("看得到");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 图读不了了：照记下的给，说明没去读图。
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("改得了");
        let again = look(&state, &path, true);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("改得回");
        assert_eq!(again.as_ref(), Some(&first), "照记下的给，不读图");
    }

    // 换了图（大小变了）：重算。
    let blue = picture(8);
    std::fs::write(&path, &blue).expect("写得进");
    let changed = look(&state, &path, true).expect("看得到");
    assert_eq!(changed.version, ContentHash::of(&blue).hex()[..16]);
    assert_eq!(changed.seeds, seeds_of(&blue));
}

#[test]
fn seeds_asked_later_are_picked_then() {
    let (_root, dir) = temp_root("glance-later");
    let state = dir.join("state");
    let path = dir.join("background.png");
    let red = picture(48);
    std::fs::write(&path, &red).expect("写得进");
    let plain = look(&state, &path, false).expect("看得到");
    assert_eq!(plain.seeds, None, "没要的不挑");
    let with = look(&state, &path, true).expect("看得到");
    assert_eq!(with.version, plain.version);
    assert_eq!(with.seeds, seeds_of(&red), "后来要了补算");
    assert!(
        look(&state, &dir.join("missing.png"), true).is_none(),
        "图不在的没有"
    );
}

#[test]
fn a_picture_of_the_same_size_is_known_by_its_modified_time() {
    let (_root, dir) = temp_root("glance-same-size");
    let state = dir.join("state");
    let path = dir.join("avatar.png");
    std::fs::write(&path, b"aaaa").expect("写得进");
    let first = look(&state, &path, false).expect("看得到");
    std::fs::write(&path, b"bbbb").expect("写得进");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .and_then(|file| file.set_modified(later))
        .expect("改得了修改时刻");
    let changed = look(&state, &path, false).expect("看得到");
    assert_ne!(changed.version, first.version, "大小一样、改过的照样重算");
    assert_eq!(changed.version, ContentHash::of(b"bbbb").hex()[..16]);
}

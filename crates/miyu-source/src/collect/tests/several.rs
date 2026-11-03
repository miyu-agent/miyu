//! 一个 Release 里有几个包（施工 G-16）：照编号分开，各自收成一条、各自占编号；被别人占了的那一个单独跳过。

use super::*;

#[test]
fn one_release_may_carry_several_packages_each_claimed_on_its_own() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let more = "targets = [\"linux-x86_64\", \"macos-aarch64\"]\n";
    let im_linux = package(
        scratch.path(),
        "miyu-im",
        "0.1.0",
        more,
        Some(Target::LinuxX86_64),
    );
    let sponsor = package(scratch.path(), "miyu-im-sponsor", "0.1.0", "", None);
    let im_mac = package(
        scratch.path(),
        "miyu-im",
        "0.1.0",
        more,
        Some(Target::MacosAarch64),
    );
    let taken = package(scratch.path(), "memes", "1.0", "", None);
    net.release("miyu-agent/miyu-im", &[im_linux, sponsor, im_mac, taken]);
    let mut rules = rules(&["miyu-agent/miyu-im"]);
    rules.names.insert(
        "memes".to_string(),
        "github.com/miyu-agent/miyu-memes".to_string(),
    );
    let got = collect(
        &rules,
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-11-04T03:00:00Z",
    );
    let ids: Vec<(&str, usize)> = got
        .index
        .packages
        .iter()
        .map(|entry| (entry.id.as_str(), entry.files.len()))
        .collect();
    assert_eq!(
        ids,
        [("miyu-im", 2), ("miyu-im-sponsor", 1)],
        "{:?}",
        got.notes
    );
    for id in ["miyu-im", "miyu-im-sponsor"] {
        assert_eq!(
            got.names.get(id).map(String::as_str),
            Some("github.com/miyu-agent/miyu-im"),
            "{id} 各自占下"
        );
    }
    let said = got.notes.join("\n");
    assert!(
        said.contains(
            "miyu-agent/miyu-im: skipped, id memes belongs to github.com/miyu-agent/miyu-memes"
        ),
        "被别人占了的那一个单独跳过，别的照收：\n{said}"
    );
}

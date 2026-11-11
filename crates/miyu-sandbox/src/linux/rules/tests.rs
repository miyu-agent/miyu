//! 两层叠起来和从头建的一层放行的一样（施工 V-47「两层」）：每一样操作，放它的路径（照先后）在从头建的那一层里和在管它的那一层
//! 里一样——「连 Unix 套接字」「跨目录改名、链接」看每条命令叠的那一小层，别的看缓存的那一层；两层各管的合起来正好是从头建的
//! 那一层管的。

use super::*;

/// 放 `right` 的规则的路径，照先后。
fn granting(rules: &[Rule], right: AccessFs) -> Vec<(PathBuf, bool)> {
    rules
        .iter()
        .filter(|rule| rule.access.contains(right))
        .map(|rule| (rule.path.clone(), rule.readable))
        .collect()
}

#[test]
fn the_two_layers_grant_exactly_what_the_whole_one_grants() {
    let spec = Spec {
        write: vec![PathBuf::from("/w/one"), PathBuf::from("/w/two.txt")],
        hidden: vec![PathBuf::from("/h")],
        read: None,
        reveal: vec![PathBuf::from("/h/skill")],
        sockets: vec![
            PathBuf::from("/run/call.sock"),
            PathBuf::from("/run/b.sock"),
        ],
    };
    let plan = vec![
        PathBuf::from("/a"),
        PathBuf::from("/b"),
        PathBuf::from("/w"),
    ];
    let mut whole = base(&spec, plan.clone(), Layer::Whole);
    whole.extend(sockets(&spec));
    let cached = base(&spec, plan, Layer::Base);
    let unix = unix(&spec);
    for right in AccessFs::from_all(LATEST) {
        let (layer, name) = match right {
            AccessFs::ResolveUnix | AccessFs::Refer => (&unix, "叠的那一小层"),
            _ => (&cached, "缓存的那一层"),
        };
        assert_eq!(
            granting(&whole, right),
            granting(layer, right),
            "{right:?} 看{name}"
        );
    }
    // 缓存的那一层一条「连 Unix 套接字」都不放，套接字不在里面。
    assert!(granting(&cached, AccessFs::ResolveUnix).is_empty());
    assert!(!cached.iter().any(|rule| rule.path.starts_with("/run")));
    // 叠的那一小层只放这两样。
    assert!(
        unix.iter()
            .all(|rule| (rule.access & !(AccessFs::ResolveUnix | AccessFs::Refer)).is_empty())
    );
}

#[test]
fn the_two_layers_handle_exactly_what_the_whole_one_handles() {
    // 管什么：缓存的那一层是从头建的去掉「连 Unix 套接字」，叠的那一小层是「连 Unix 套接字」和「跨目录改名、链接」——后一样两层都管，
    // 两层都放在同样的路径上（上一个测试）。
    let whole = AccessFs::from_all(LATEST);
    let cached = whole & !AccessFs::ResolveUnix;
    let unix = AccessFs::ResolveUnix | AccessFs::Refer;
    assert_eq!(cached | unix, whole);
    assert_eq!(cached & unix, BitFlags::from(AccessFs::Refer));
}

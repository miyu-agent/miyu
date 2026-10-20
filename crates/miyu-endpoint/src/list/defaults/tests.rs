use super::*;

fn id(n: u8) -> SessionId {
    SessionId::parse(&format!("0199a1b2-0000-7000-8000-0000000000{n:02x}")).expect("合写法")
}

#[test]
fn a_child_and_a_grandchild_reach_the_main_session() {
    let parents = BTreeMap::from([
        (id(1), None),
        (id(2), Some(id(1))),
        (id(3), Some(id(2))),
        (id(4), Some(id(9))),
    ]);
    assert_eq!(main_of(&parents, &id(1)), id(1));
    assert_eq!(main_of(&parents, &id(3)), id(1));
    assert_eq!(main_of(&parents, &id(4)), id(9), "父会话不在索引里：停在它");
    let looped = BTreeMap::from([(id(5), Some(id(6))), (id(6), Some(id(5)))]);
    assert_eq!(main_of(&looped, &id(5)), id(6), "绕回来的照走过的挡下");
}

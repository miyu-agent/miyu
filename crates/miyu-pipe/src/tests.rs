use super::*;

#[test]
fn only_the_owner_gets_in() {
    assert_eq!(
        owner_only("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)"
    );
}

#[test]
fn containers_only_get_to_connect() {
    assert_eq!(
        reachable_from_containers("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)(A;;0x100083;;;AC)"
    );
}

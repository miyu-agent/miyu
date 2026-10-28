//! 交接票（接入通讯平台、`miyu web` 要的）：一张只换一次；过期的不认；最多 16 张，多了丢最早的。

use super::*;

#[test]
fn a_ticket_is_taken_once() {
    let tickets = Tickets::default();
    let later = Instant::now() + TICKET_TTL;
    tickets.put("t1".to_string(), later);
    assert!(!tickets.take("t2"));
    assert!(tickets.take("t1"));
    assert!(!tickets.take("t1"), "只换一次");
}

#[test]
fn expired_tickets_are_refused_and_only_sixteen_are_kept() {
    let tickets = Tickets::default();
    tickets.put("old".to_string(), Instant::now());
    std::thread::sleep(Duration::from_millis(5));
    assert!(!tickets.take("old"), "过期的不认");
    let later = Instant::now() + TICKET_TTL;
    for n in 0..17 {
        tickets.put(format!("t{n}"), later);
    }
    assert!(!tickets.take("t0"), "多了丢最早的");
    assert!(tickets.take("t1"));
    assert!(tickets.take("t16"));
}

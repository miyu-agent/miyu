//! 交接票（接入通讯平台、`miyu web` 要的）：一张只换一次；过期的不认；最多 16 张，多了丢最早的。

use super::*;

#[test]
fn a_ticket_is_taken_once() {
    let tickets = Tickets::default();
    let later = Instant::now() + TICKET_TTL;
    tickets.put("t1".to_string(), later, Reach::Anywhere);
    assert_eq!(tickets.take("t2"), None);
    assert_eq!(tickets.take("t1"), Some(Reach::Anywhere));
    assert_eq!(tickets.take("t1"), None, "只换一次");
}

#[test]
fn a_ticket_remembers_where_it_may_be_taken() {
    let tickets = Tickets::default();
    let later = Instant::now() + TICKET_TTL;
    tickets.put("page".to_string(), later, Reach::Here);
    tickets.put("person".to_string(), later, Reach::Anywhere);
    assert_eq!(
        tickets.take("page"),
        Some(Reach::Here),
        "扩展替本机页面要的"
    );
    assert_eq!(tickets.take("person"), Some(Reach::Anywhere));
}

#[test]
fn expired_tickets_are_refused_and_only_sixteen_are_kept() {
    let tickets = Tickets::default();
    tickets.put("old".to_string(), Instant::now(), Reach::Anywhere);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(tickets.take("old"), None, "过期的不认");
    let later = Instant::now() + TICKET_TTL;
    for n in 0..17 {
        tickets.put(format!("t{n}"), later, Reach::Anywhere);
    }
    assert_eq!(tickets.take("t0"), None, "多了丢最早的");
    assert!(tickets.take("t1").is_some());
    assert!(tickets.take("t16").is_some());
}

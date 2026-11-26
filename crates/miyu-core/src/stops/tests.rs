use super::*;

/// 装不上的信号当它不会来（施工 4-9 再补三上）：一直不回；装上了又断了的也是。来了的照常回。
#[tokio::test]
async fn a_signal_that_could_not_be_watched_never_arrives() {
    let never = arrived(None::<std::future::Ready<Option<()>>>);
    let waited = tokio::time::timeout(Duration::from_millis(50), never).await;
    assert!(waited.is_err(), "装不上的不当成收到了");
    let closed = arrived(Some(std::future::ready(None)));
    let waited = tokio::time::timeout(Duration::from_millis(50), closed).await;
    assert!(waited.is_err(), "断了的不当成收到了");
    let came = arrived(Some(std::future::ready(Some(()))));
    let waited = tokio::time::timeout(Duration::from_secs(5), came).await;
    assert!(waited.is_ok(), "来了的照常回");
}

/// 收尾的总时限：比 `miyu core stop` 等核心退的 60 秒、systemd 默认的 90 秒都短。
#[test]
fn stopping_is_given_thirty_seconds() {
    assert_eq!(LIMIT, Duration::from_secs(30));
}

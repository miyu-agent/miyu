//! 抽出来的几条照说话的人认是谁（施工 E-1 上、E-1 修四）：对上的换成平台编号、说的是主人的改记成 `user`、对不上的照名字；
//! 名字只写了前半截的照恰好一个人认；发生过的事涉及谁同样认；关于主人的一条说的是她自己的改记成 `episode`。名字、编号都是编的。

use super::*;

fn one(class: &str, who: Option<&str>) -> Candidate {
    Candidate {
        class: class.into(),
        text: "一件事".into(),
        turn: 5,
        about: None,
        who: who.map(str::to_string),
        people: Vec::new(),
    }
}

fn names(pairs: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
    pairs
        .iter()
        .map(|(name, id)| (name.to_string(), id.map(str::to_string)))
        .collect()
}

fn me() -> BTreeSet<String> {
    BTreeSet::from(["miyu".to_string()])
}

fn placed(found: &[Candidate]) -> Vec<(&str, Option<&str>)> {
    found
        .iter()
        .map(|one| (one.class.as_str(), one.who.as_deref()))
        .collect()
}

#[test]
fn people_are_placed_by_who_spoke() {
    let names = names(&[("小林", Some("qq:2")), ("Owner", None)]);
    let mut found = vec![
        one("person", Some("小林")),
        one("person", Some("Owner")),
        one("person", Some("老王")),
        one("user", Some("小林")),
    ];
    place_people(&mut found, &names, &BTreeSet::new());
    assert_eq!(
        placed(&found),
        [
            ("person", Some("qq:2")),
            ("user", None),
            ("person", Some("老王")),
            ("user", None),
        ]
    );
}

/// 群名片后面带着一长串说明，模型只写了前半截：是这一段里恰好一个人的名字的开头的认成他；对上两个人的、太短的、对上平台编号
/// 本身的、对上她的名字的不认，照名字留。
#[test]
fn a_name_cut_short_is_placed_by_the_one_it_begins() {
    let names = names(&[
        ("AAA卢关王(你群最有情怀的人)", Some("qq:17")),
        ("qq:17", Some("qq:17")),
        ("阿杰A", Some("qq:21")),
        ("阿杰B", Some("qq:22")),
        ("Miyu酱", Some("qq:30")),
        ("Owner", None),
        ("阿楠(群主)", None),
    ]);
    let mut found = vec![
        one("person", Some(" aaa卢关王 ")),
        one("person", Some("阿杰")),
        one("person", Some("A")),
        one("person", Some("qq:1")),
        one("person", Some("Miyu酱")),
        one("person", Some("阿楠")),
    ];
    place_people(&mut found, &names, &me());
    assert_eq!(
        placed(&found),
        [
            ("person", Some("qq:17")),
            ("person", Some("阿杰")),
            ("person", Some("A")),
            ("person", Some("qq:1")),
            ("person", Some("qq:30")),
            ("user", None),
        ]
    );
}

/// 她的名字不拿来照开头认人：「Miyu」不认成用过「Miyu酱」的那个人，先被挑出来记成她的事；只写了「Miy」的也不认成把群名片
/// 改成「Miyu」的那个群友。
#[test]
fn her_name_is_never_the_head_of_someone_else() {
    let names = names(&[("Miyu酱", Some("qq:30")), ("Miyu", Some("qq:31"))]);
    let mut found = vec![one("person", Some("Miyu")), one("person", Some("Miy"))];
    place_people(&mut found, &names, &me());
    assert_eq!(placed(&found), [("episode", None), ("person", Some("Miy"))]);
}

/// 发生过的事涉及谁照同样的认法：对上的换成平台编号、名字只写了前半截的照恰好一个人、她自己和主人不算、对不上的照名字、去重；
/// 别的类不带涉及谁。
#[test]
fn an_episode_names_who_it_involves() {
    let names = names(&[
        ("小林", Some("qq:2")),
        ("林林子(摄影)", Some("qq:2")),
        ("阿杰", Some("qq:3")),
        ("Owner", None),
    ]);
    let mut episode = one("episode", None);
    episode.people = ["小林", "Miyu", "Owner", "林林子", "小雨", "阿杰", "qq:3"]
        .map(String::from)
        .to_vec();
    let mut user = one("user", None);
    user.people = vec!["小林".into()];
    let mut found = vec![episode, user];
    place_people(&mut found, &names, &me());
    assert_eq!(found[0].people, ["qq:2", "小雨", "qq:3"]);
    assert!(found[1].people.is_empty(), "别的类不带涉及谁");
}

/// 关于主人的一条照她的名字起头的（模型照 `You` 写成了「我是 Miyu」）是她自己的事：改记成 `episode`，涉及谁照发生过的事认。
/// 照别人的名字起头、名字只是她名字开头的（Miyuki）照旧。
#[test]
fn a_user_memory_about_her_becomes_an_episode() {
    let names = names(&[("小林", Some("qq:2"))]);
    let texts = [
        "我是 Miyu，QQ 群 AI 助手",
        "Miyu 没有群里的禁言权",
        "I'm Miyu, a bot",
        "Miyuki 喜欢猫",
        "阿楠说 Miyu 很可爱",
    ];
    let mut found: Vec<Candidate> = texts
        .iter()
        .map(|text| Candidate {
            text: (*text).into(),
            ..one("user", None)
        })
        .collect();
    found[0].people = vec!["小林".into()];
    place_people(&mut found, &names, &me());
    let classes: Vec<&str> = found.iter().map(|one| one.class.as_str()).collect();
    assert_eq!(classes, ["episode", "episode", "episode", "user", "user"]);
    assert_eq!(found[0].people, ["qq:2"], "改记成发生过的事的照样认涉及谁");
    let mut nameless = vec![Candidate {
        text: "我是 Miyu".into(),
        ..one("user", None)
    }];
    place_people(&mut nameless, &names, &BTreeSet::new());
    assert_eq!(nameless[0].class, "user", "不知道她叫什么的照旧");
}

/// 施工 E-1 修五：模型写的是谁带着看不见的双向格式字符、在名字后面加了括号注释、写了半截又接省略号、括号没写完的，清洗以后
/// 认成说话的人；群名片自己带着这些字的也照清洗过的比。
#[test]
fn a_name_with_invisible_marks_or_a_note_is_placed() {
    let names = names(&[
        ("/败", Some("qq:9")),
        ("xnijsns (\"\u{2067}\u{2067}彩虹", Some("qq:8")),
        ("阿岩\u{200F}", Some("qq:7")),
        ("长名片后面一长串说明", Some("qq:3")),
        ("Owner", None),
    ]);
    let mut found = vec![
        one("person", Some("/败（QQ 群昵称）")),
        one("person", Some("xnijsns (\"\u{2067}\u{2067} …")),
        one("person", Some("阿岩")),
        one("person", Some("\u{202E}小林\u{202C}")),
        one("person", Some("长名片…")),
    ];
    place_people(&mut found, &names, &me());
    assert_eq!(
        placed(&found),
        [
            ("person", Some("qq:9")),
            ("person", Some("qq:8")),
            ("person", Some("qq:7")),
            ("person", Some("小林")),
            ("person", Some("qq:3")),
        ],
        "认得出的换成编号，认不出的照名字留、只去看不见的字"
    );
}

/// 施工 E-1 修五：括号是名字自己的一部分时照原样认；清洗以后对不上谁的照原样留（只去看不见的字）。
#[test]
fn cleaning_does_not_spoil_a_plain_name() {
    let names = names(&[("猫(Cat)", Some("qq:4")), ("猫", Some("qq:6"))]);
    let mut found = vec![
        one("person", Some("猫(Cat)")),
        one("person", Some("猫")),
        one("person", Some("狗（备注）")),
    ];
    place_people(&mut found, &names, &me());
    assert_eq!(
        placed(&found),
        [
            ("person", Some("qq:4")),
            ("person", Some("qq:6")),
            ("person", Some("狗（备注）")),
        ]
    );
}

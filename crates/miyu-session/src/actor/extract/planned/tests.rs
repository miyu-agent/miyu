//! 从一处读起拼的和整份读拼的一个字节不差（施工 E-5 补）：群会话里新话少、名字只在更早的日志里出现过、换人格、压缩、读起的那
//! 一处跨段、核心刚起（还没接上）都一样；读起以后有撤销的整份读。名字、群名、群号都是编的。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, Seq};
use miyu_kernel::time::UtcOffset;
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SessionLog, read_events};
use miyu_store::personas::Personas;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use super::{Planning, planned};
use crate::memory::{ExtractTexts, Extractor, Landings, Plan, Read, ReadFrom, read_extract};
use crate::snapshot::{Parts, build};

/// 临时目录：用完删掉。
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        miyu_store::testkit::remove_tree(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!("miyu-e5b-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    std::fs::create_dir_all(&dir).expect("建得了临时目录");
    // 会话日志只在数据根里建目录（施工 TMP-1）：写上标记。
    std::fs::write(dir.join(miyu_store::root::MARKER), "").expect("写得进标记");
    Scratch(dir)
}

fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 群会话的组装器：照随核心附带的 Miyu 拼一份群会话的快照。
fn group_assembler(scratch: &Scratch) -> Arc<dyn Assembler + Send + Sync> {
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    let prompts = root.path().join("home/alice/personas/miyu/prompts");
    std::fs::create_dir_all(&prompts).expect("建得了人格目录");
    std::fs::write(prompts.join("persona.md"), "你是 Miyu。").expect("写得进");
    let alice = AccountId::parse("alice").expect("账号合写法");
    let resources = resources();
    let personas = Personas::new(&resources, &root, &alice);
    let parts = Parts {
        name: Some("miyu".to_string()),
        texts: personas.find("miyu").expect("找得到 Miyu").texts,
        attended: false,
        face: Vec::new(),
        memory: None,
        child: false,
        preset: None,
        roleplay: true,
        group: Some((540, None)),
        foreground: false,
        show_hint: true,
        knowledge: None,
        skills: None,
        profile: None,
        project: None,
        appended: Default::default(),
        bare: false,
        plugin_tools: Vec::new(),
    };
    let snapshot = build(&resources, parts).expect("拼得成");
    snapshot.policy().expect("造得出策略").assembler
}

/// 照先后造事件：序号、时刻接着往下数。
struct Log {
    seq: u64,
    minute: u64,
    events: Vec<Event>,
}

impl Log {
    fn new() -> Log {
        let mut log = Log {
            seq: 0,
            minute: 0,
            events: Vec::new(),
        };
        log.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"session.created","by":{{"kind":"person","account":"alice"}},"cause":"c-1","body":{{"owner":"im","venue":"qq:group:5550","policy":"sha256:{}","permission":{{"level":"workspace","read_only":false}},"model":"fake/model"}}}}"#,
                "0".repeat(64)
            )
        });
        log
    }

    fn push(&mut self, line: impl FnOnce(u64, String) -> String) -> u64 {
        self.seq += 1;
        self.minute += 1;
        let at = format!(
            "2026-10-2{}T{:02}:{:02}:00.000Z",
            1 + self.minute / 1440,
            (self.minute / 60) % 24,
            self.minute % 60
        );
        let text = line(self.seq, at);
        self.events
            .push(Event::from_line(&text).unwrap_or_else(|e| panic!("{e}: {text}")));
        self.seq
    }

    /// 群里一个人说一句：平台编号 `id`、群名片 `name`，她在群里叫 `my_name`、群叫 `place`（有的才写）。
    fn said(
        &mut self,
        id: u64,
        name: &str,
        text: &str,
        my_name: Option<&str>,
        place: Option<&str>,
    ) {
        let extra = my_name
            .map(|my| format!(r#","my_name":"{my}""#))
            .unwrap_or_default()
            + &place
                .map(|place| format!(r#","place":"{place}""#))
                .unwrap_or_default();
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5550","id":"qq:{id}","role":"member"}},"cause":"m-{seq}","body":{{"blocks":[{{"type":"text","text":"{text}"}}],"venue":{{"msg":"{seq}","name":"{name}"{extra}}}}}}}"#
            )
        });
    }

    /// 主人在群里说一句（`by` 是账号：撤掉那一轮时这句跟着撤），群名片是 `name`。
    fn owner_said(&mut self, name: &str, text: &str) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"person","account":"alice"}},"cause":"o-{seq}","body":{{"blocks":[{{"type":"text","text":"{text}"}}],"venue":{{"msg":"{seq}","name":"{name}"}}}}}}"#
            )
        });
    }

    /// 她答一轮：回合开始、（要的话先调一次 remember）一句回答、回合结束。交回回答那一条的序号。
    fn answer(&mut self, text: &str, remember: bool) -> u64 {
        let trigger = self.seq;
        let turn = self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"cause":"t-{seq}","body":{{"trigger":{trigger}}}}}"#
            )
        });
        if remember {
            self.push(|seq, at| {
                let seen = seq - 1;
                format!(
                    r#"{{"seq":{seq},"at":"{at}","kind":"message.assistant","turn":{turn},"by":{{"kind":"model","endpoint":"fake","model":"model"}},"cause":"t-{turn}","body":{{"blocks":[{{"type":"tool_call","call_id":"call_{seq}_1","name":"remember","args":"{{}}"}}],"seen":{seen}}}}}"#
                )
            });
        }
        let answer = self.push(|seq, at| {
            let seen = seq - 1;
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.assistant","turn":{turn},"by":{{"kind":"model","endpoint":"fake","model":"model"}},"cause":"t-{turn}","body":{{"blocks":[{{"type":"text","text":"{text}"}}],"seen":{seen}}}}}"#
            )
        });
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"turn.ended","turn":{turn},"by":{{"kind":"kernel"}},"cause":"t-{turn}","body":{{"reason":"completed"}}}}"#
            )
        });
        answer
    }

    /// 她派一个子代理（在会话 `session` 里跑）：回合开始、派出去的结果、一句回答、回合结束。
    fn dispatched(&mut self, session: &str) {
        let trigger = self.seq;
        let turn = self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"cause":"t-{seq}","body":{{"trigger":{trigger}}}}}"#
            )
        });
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"tool.result","turn":{turn},"by":{{"kind":"tool","call_id":"call_{turn}_1"}},"cause":"t-{turn}","body":{{"call_id":"call_{turn}_1","status":"ok","blocks":[{{"type":"text","text":"Started subagent j1."}}],"duration_ms":5,"effects":[{{"kind":"job.started","job":"j1","what":"agent","title":"查一下","session":"{session}"}}]}}}}"#
            )
        });
        self.push(|seq, at| {
            let seen = seq - 1;
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.assistant","turn":{turn},"by":{{"kind":"model","endpoint":"fake","model":"model"}},"cause":"t-{turn}","body":{{"blocks":[{{"type":"text","text":"我让它去查。"}}],"seen":{seen}}}}}"#
            )
        });
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"turn.ended","turn":{turn},"by":{{"kind":"kernel"}},"cause":"t-{turn}","body":{{"reason":"completed"}}}}"#
            )
        });
    }

    /// 会话 `session`（她派的子代理）发来一句。
    fn reported(&mut self, session: &str, text: &str) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"session","id":"{session}"}},"cause":"{session}/message/1","body":{{"blocks":[{{"type":"text","text":"{text}"}}]}}}}"#
            )
        });
    }

    fn persona(&mut self, persona: &str) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"session.policy_changed","by":{{"kind":"person","account":"alice"}},"cause":"p-{seq}","body":{{"persona":"{persona}"}}}}"#
            )
        });
    }

    fn compacted(&mut self) {
        let upto = self.seq;
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"context.compacted","by":{{"kind":"kernel"}},"cause":"k-{seq}","body":{{"upto":{upto},"summary":"摘要。"}}}}"#
            )
        });
    }

    fn reverted(&mut self, turn: u64) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"turn.reverted","by":{{"kind":"person","account":"alice"}},"cause":"r-{seq}","body":{{"turns":[{turn}]}}}}"#
            )
        });
    }

    /// 造到现在、还没交出去的那几条。
    fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

/// 会话落一批：照会话那样先交给抽取状态折进来，写进日志，再记下落完照到哪。
fn land(log: &mut SessionLog, extractor: &Extractor, batch: &[Event]) {
    extractor.note_prefix(batch, true);
    log.append(batch).expect("写得进");
    extractor.landed(log.mark());
}

/// 群里热闹的一轮：几个人说几句（第 `round` 轮），她答。交回她那一条的序号。
fn busy_round(log: &mut Log, round: u64) -> u64 {
    let members = [
        (10_001, "小林"),
        (20_017, "阿杰"),
        (30_002, "Aki"),
        (40_404, "老周"),
    ];
    for line in 0..(1 + round % 4) {
        let (id, name) = members[usize::try_from((round + line) % 4).expect("小")];
        // 小林第 30 轮起改了群名片；她在群里的名字隔几轮带一次，群名第 25 轮改过。
        let name = if id == 10_001 && round >= 30 {
            "林小林"
        } else {
            name
        };
        let my_name = round.is_multiple_of(5).then_some("米羽");
        let place = round.is_multiple_of(3).then_some(if round < 25 {
            "周末群"
        } else {
            "周末摄影群"
        });
        log.said(id, name, &format!("第{round}轮第{line}句"), my_name, place);
    }
    log.answer(&format!("好的{round}。"), round % 11 == 7)
}

struct Fixture {
    _scratch: Scratch,
    dir: PathBuf,
    assembler: Arc<dyn Assembler + Send + Sync>,
    texts: ExtractTexts,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let scratch = scratch(name);
        let assembler = group_assembler(&scratch);
        let texts =
            ExtractTexts::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
                .expect("读得到");
        Fixture {
            dir: scratch.0.join("session"),
            _scratch: scratch,
            assembler,
            texts,
        }
    }

    fn planning(&self) -> Planning<'_> {
        Planning {
            assembler: self.assembler.as_ref(),
            budget: Some(160),
            limit: Some(3),
            min_turns: 1,
            offset: UtcOffset::UTC,
            texts: &self.texts,
            name: Some("Shorin"),
        }
    }

    /// 整份读拼的。
    fn whole(&self, after: u64) -> Plan {
        let events = read_events(&self.dir).expect("读得了");
        planned(
            Read {
                after: Seq::new(after).expect("从 1 起"),
                events,
                from: ReadFrom::Start(None),
            },
            &self.planning(),
        )
    }

    /// 照抽取状态读（还接不上的顺手接上）拼的，和整份读拼的比；交回是不是从一处读起的。
    fn same(&self, extractor: &Extractor, after: u64) -> bool {
        self.same_from(extractor, &extractor.landings(), after)
    }

    /// 同 [`Fixture::same`]，照 `landings`（抽的那一刻拿到的几处）读。从一处读起的挑的是最后一处：读起的那一条离上次抽到的
    /// 那一条不远（前一轮结束以后）。
    fn same_from(&self, extractor: &Extractor, landings: &Landings, after: u64) -> bool {
        let mut read =
            read_extract(&self.dir, Seq::new(after).expect("从 1 起"), landings).expect("读得了");
        if matches!(read.from, ReadFrom::Mark(_)) {
            let first = read.events.first().map_or(after, |event| event.seq.get());
            assert!(
                first <= after + 1 && first + 20 > after,
                "从上次抽到的那一条以前最后一处读起：第 {first} 条起，上次抽到第 {after} 条"
            );
        }
        if let ReadFrom::Start(landed) = &mut read.from
            && let Some(landed) = landed.take()
        {
            extractor.seed(landed);
        }
        let marked = matches!(read.from, ReadFrom::Mark(_));
        let fast = planned(read, &self.planning());
        assert!(matches!(fast, Plan::Ask { .. }), "抽到了东西：{fast:?}");
        assert_eq!(fast, self.whole(after), "上次抽到第 {after} 条");
        marked
    }
}

/// 她派的子代理在哪个会话里跑。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 一直热闹的群会话：接上以后每次都从一处读起，拼出来的和整份读的一样；名字只在更早出现过的人照样认得出，群名片改了照新的，
/// 换人格、压缩、读起以前派的子代理读起以后才报回来也一样；读起的那一处跨了段的也一样。
#[test]
fn reading_from_a_mark_plans_the_same_as_reading_everything() {
    let fixture = Fixture::new("same");
    // 段很小：几轮就换一段，读起的那一处常在前一段里。
    let mut log = SessionLog::create(&fixture.dir, 4096).expect("建得了日志");
    let extractor = Extractor::default();
    let mut events = Log::new();
    // 只在头几轮说话的人：后面抽的时候认名字要靠读起以前的那一截。
    events.said(77_777, "早来的人", "大家好", None, Some("周末群"));
    let mut answers = Vec::new();
    let mut owners = 0;
    for round in 0..60 {
        if round == 3 {
            // 主人用一个只出现这一次的群名片说一句、她答：之后撤掉这一轮。
            events.owner_said("店主老王", "我先走了");
            owners = events.answer("慢走。", false) - 1;
        }
        if round == 6 {
            events.dispatched(CHILD);
        }
        if round == 12 {
            events.persona("miyu");
        }
        if round == 40 {
            // 读起以后子代理才报回来：照派它的那一条认出是子代理，不当成别的会话。
            events.reported(CHILD, "查完了，是配置写错了");
        }
        if round == 33 {
            events.compacted();
        }
        answers.push(busy_round(&mut events, round));
        land(&mut log, &extractor, &events.take());
        if round == 4 {
            // 头一次抽：还接不上，整份读、顺手接上。
            assert!(!fixture.same(&extractor, answers[1]), "头一次整份读");
        }
    }
    for k in [8, 19, 20, 31, 34, 39, 45, 57] {
        assert!(
            fixture.same(&extractor, answers[k]),
            "第 {k} 轮以后从一处读起"
        );
    }
    // 读起以后有撤销的（撤掉读起以前、主人用另一个群名片说的那一轮）：抽的那一刻拿到的几处还在，读到撤销就整份读，拼出来
    // 照样一样（照撤掉以后认名字）；之后一直整份读。
    let stale = extractor.landings();
    events.reverted(owners);
    land(&mut log, &extractor, &events.take());
    assert!(
        !fixture.same_from(&extractor, &stale, answers[45]),
        "读起以后有撤销的整份读"
    );
    assert!(!fixture.same(&extractor, answers[45]), "有撤销的整份读");
    answers.push(busy_round(&mut events, 60));
    land(&mut log, &extractor, &events.take());
    assert!(
        !fixture.same(&extractor, answers[55]),
        "接不上了，一直整份读"
    );
}

/// 核心刚起（还不知道更早的日志里有什么）：落的批先记着；整份读回来接上，读的时候又落的批照记下的折进去；之后从一处读起，拼出来
/// 一样。
#[test]
fn a_reloaded_session_catches_up_with_one_whole_read() {
    let fixture = Fixture::new("reload");
    let mut log = SessionLog::create(&fixture.dir, 1 << 20).expect("建得了日志");
    let mut events = Log::new();
    events.said(77_777, "早来的人", "大家好", None, None);
    let mut answers = Vec::new();
    // 上一回核心在的时候落的：这一回的抽取状态不知道。
    for round in 0..20 {
        answers.push(busy_round(&mut events, round));
        log.append(&events.take()).expect("写得进");
    }
    let extractor = Extractor::default();
    for round in 20..23 {
        answers.push(busy_round(&mut events, round));
        land(&mut log, &extractor, &events.take());
    }
    // 整份读的那一刻……
    let mut read = read_extract(
        &fixture.dir,
        Seq::new(answers[10]).expect("从 1 起"),
        &extractor.landings(),
    )
    .expect("读得了");
    // ……以后又落了两批，才交回来接上：头一句是个只说这一次的人，接上时照记下的折进去才认得出。
    events.said(88_888, "新来的人", "我来了", None, None);
    for round in 23..25 {
        answers.push(busy_round(&mut events, round));
        land(&mut log, &extractor, &events.take());
    }
    let ReadFrom::Start(landed) = &mut read.from else {
        panic!("还接不上的整份读");
    };
    extractor.seed(landed.take().expect("顺手认出了末尾那一处"));
    for round in 25..40 {
        answers.push(busy_round(&mut events, round));
        land(&mut log, &extractor, &events.take());
    }
    for k in [24, 26, 33] {
        assert!(
            fixture.same(&extractor, answers[k]),
            "接上以后第 {k} 轮从一处读起"
        );
    }
}

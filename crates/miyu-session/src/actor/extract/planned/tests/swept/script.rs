//! 照种子造抽取的对照测试用的会话（施工 V-43）：底子是撤销、恢复的对照测试的那一套（`crate::testkit::undo_log`：撤销、恢复、
//! 重做、撤回、压缩、撤销跨压缩、分叉、回报、子代理），再照抽取要看的改几样：她的回复字各不一样（偶尔空的、很长的）、偶尔带思考、
//! 偶尔调 `remember`、`forget`；人话字各不一样，群会话里多半换成群友（平台编号、群名片隔一阵换一次、偶尔带她在群里叫什么和群名），
//! 私聊里偶尔是平台上的人；不认识的种类换成别的线替她发进群里的话；子代理的回报偶尔换成子代理发来的话；偶尔是子会话（造会话的
//! 那一条写了父会话）。偶尔把几条回复的 `seen` 改乱、改成和上一条回复的一样（没看到上一条回复；都过不了账本，外人改过的那种），
//! 逼出投影切段的边界不照先后的、一段里有两条回复的。

use serde_json::{Value, json};

use crate::testkit::undo_log::{Dice, UndoLog};

/// 这条线自己（同 `planned/tests.rs` 的 `MAIN`）。
pub(super) const MAIN: &str = "01a17c04-762d-7174-8307-fd1c42c011b8";

/// 别的线（支线）。
const BRANCH: &str = "01a17c05-26da-71a1-9c5c-91050118886b";

/// 父会话（子会话的造会话那一条写它）。
const PARENT: &str = "01a0cf12-7e40-7d1b-8c3a-51d49f03b21c";

/// 群友：平台编号、群名片的几种写法（隔一阵换一次）。
const MEMBERS: [(u64, [&str; 3]); 5] = [
    (10_001, ["小林", "林小林", "Lin"]),
    (20_017, ["阿杰", "杰哥", "阿杰"]),
    (30_002, ["Aki", "aki", "秋"]),
    (40_404, ["老周", "周", "老周头"]),
    (77_777, ["早来的人", "早来的人", "走了的人"]),
];

/// 她在群里被叫的几种、群名的几种。
const MY_NAMES: [&str; 3] = ["米羽", "Mi", " miyu "];
const PLACES: [&str; 3] = ["周末群", "周末摄影群", "摄影群"];

/// 造好的一段。
pub(super) struct Script {
    /// 一行一条。
    pub(super) lines: Vec<String>,
    /// 第一条的序号（分叉出来的不是 1）。
    pub(super) first: u64,
    /// 群会话。
    pub(super) group: bool,
    /// 她的回复的序号：上次抽到的多半落在这里。
    pub(super) replies: Vec<u64>,
}

/// 照种子造一段。
pub(super) fn script(dice: &mut Dice) -> Script {
    let forked = dice.odds(6).then(|| {
        let upto = 6 + dice.below(40);
        (upto, vec![2, 3 + dice.below(upto - 3)])
    });
    let long = dice.odds(8);
    let steps = 30 + dice.below(if long { 1500 } else { 360 });
    let log = UndoLog::run(dice, steps, forked);
    let group = dice.odds(2);
    let mut dressing = Dressing {
        group,
        wild: dice.odds(5),
        lag: dice.odds(8),
        last_seen: None,
        child: log.first == 1 && dice.odds(5),
        epoch: 20 + dice.below(120),
        first: log.first,
    };
    let mut replies = Vec::new();
    let lines = log
        .lines
        .iter()
        .map(|line| {
            let (line, reply) = dressing.dress(line, dice);
            if let Some(seq) = reply {
                replies.push(seq);
            }
            line
        })
        .collect();
    Script {
        lines,
        first: log.first,
        group,
        replies,
    }
}

/// 照抽取要看的改几样。
struct Dressing {
    group: bool,
    /// 改乱几条回复的 `seen`。
    wild: bool,
    /// 几条回复的 `seen` 照上一条回复的写（没看到上一条回复，边界照旧照先后越来越大）。
    lag: bool,
    /// 上一条回复的 `seen`。
    last_seen: Option<u64>,
    /// 子会话：造会话的那一条写了父会话。
    child: bool,
    /// 群名片、她被叫的、群名隔多少条换一次。
    epoch: u64,
    first: u64,
}

impl Dressing {
    /// 改一行；她的回复交回它的序号。
    fn dress(&mut self, line: &str, dice: &mut Dice) -> (String, Option<u64>) {
        let mut event: Value = serde_json::from_str(line).expect("造的行合 JSON");
        let seq = event["seq"].as_u64().expect("有序号");
        let kind = event["kind"].as_str().unwrap_or_default().to_string();
        let mut reply = None;
        match kind.as_str() {
            "session.created" if self.child => {
                event["body"]["parent"] = json!(PARENT);
            }
            "message.user" => self.said(&mut event, seq, dice),
            "message.assistant" => {
                self.replied(&mut event, seq, dice);
                reply = Some(seq);
            }
            "ext.note" => {
                let line = if dice.odds(3) { MAIN } else { BRANCH };
                event["kind"] = json!("venue.delivered");
                event["by"] = json!({"kind": "module", "id": "im"});
                event["body"] = json!({"line": line, "turn": 3, "to": ["qq:10001"],
                    "msg": format!("d{seq}"), "text": format!("替她发的第{seq}句")});
            }
            "child.reported" if dice.odds(2) => {
                event["kind"] = json!("message.user");
                event["body"] =
                    json!({"blocks": [{"type": "text", "text": format!("子代理说{seq}")}]});
            }
            _ => {}
        }
        (event.to_string(), reply)
    }

    /// 人话：字各不一样；群里多半换成群友，私聊里偶尔是平台上的人。
    fn said(&self, event: &mut Value, seq: u64, dice: &mut Dice) {
        if let Some(text) = event["body"]["blocks"][0]["text"].as_str()
            && !dice.odds(8)
        {
            event["body"]["blocks"][0]["text"] = json!(format!("{text}·{seq}"));
        }
        if event["by"]["kind"] != "person" && event["by"]["kind"] != "external" {
            return;
        }
        let era = (seq / self.epoch) as usize;
        let (id, names) = MEMBERS[dice.below(MEMBERS.len() as u64) as usize];
        let name = names[era % 3];
        let mut venue = json!({"msg": seq.to_string()});
        if dice.odds(3) {
            venue["my_name"] = json!(MY_NAMES[era % 3]);
        }
        if dice.odds(3) {
            venue["place"] = json!(PLACES[(era / 2) % 3]);
        }
        match (self.group, event["by"]["kind"] == "person") {
            (true, true) if !dice.odds(3) => {
                event["by"] = json!({"kind": "external", "venue": "qq:group:1",
                    "id": format!("qq:{id}"), "role": "member"});
                venue["name"] = json!(name);
                event["body"]["venue"] = venue;
            }
            (true, _) => {
                // 主人在群里、本来就是群友的：群名片偶尔写。
                if dice.odds(2) {
                    venue["name"] = json!(format!("店主{}", era % 2));
                }
                event["body"]["venue"] = venue;
            }
            (false, true) if dice.odds(6) => {
                event["by"] = json!({"kind": "external", "venue": "qq:private:9",
                    "id": format!("qq:{id}"), "role": "member"});
                if !dice.odds(4) {
                    venue["name"] = json!(name);
                }
                event["body"]["venue"] = venue;
            }
            _ => {}
        }
    }

    /// 她的回复：字各不一样，偶尔空的、很长的；偶尔调 `remember`、`forget`；外人改过的偶尔 `seen` 乱写。
    fn replied(&mut self, event: &mut Value, seq: u64, dice: &mut Dice) {
        let Some(blocks) = event["body"]["blocks"].as_array_mut() else {
            return;
        };
        if dice.odds(3) {
            blocks.insert(
                0,
                json!({"type": "reasoning", "text": format!("想想第{seq}条")}),
            );
        }
        for block in blocks.iter_mut() {
            match block["type"].as_str() {
                Some("text") => {
                    block["text"] = json!(match dice.below(12) {
                        0 => String::new(),
                        1 => format!(
                            "{}{seq}",
                            "很长的回答。".repeat(300 + dice.below(2000) as usize)
                        ),
                        _ => format!("答{seq}"),
                    });
                }
                Some("tool_call") => match dice.below(10) {
                    0 => block["name"] = json!("remember"),
                    1 => block["name"] = json!("forget"),
                    _ => {}
                },
                _ => {}
            }
        }
        if self.wild && dice.odds(3) {
            event["body"]["seen"] = json!(self.first + dice.below(seq - self.first));
        }
        if let Some(last) = self.last_seen.filter(|_| self.lag && dice.odds(2)) {
            event["body"]["seen"] = json!(last);
        }
        self.last_seen = event["body"]["seen"].as_u64();
    }
}

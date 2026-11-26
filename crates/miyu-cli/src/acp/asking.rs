//! 会话要问人的时候（设计 73 第五节第 4 条，`internal/blueprint/cli/acp.md`「确认和提问」）：工具确认翻成
//! `session/request_permission`，选项照核心给的（仅允许本次、带了放行范围的再有「在本会话内允许」、拒绝），编辑器选了哪一项照着
//! `session.answer` 回核心；认不得的、取消了的一律当拒绝。提问（`ask_user`）编辑器能弹表单的走 `elicitation/create`（补充的字也
//! 收），不能的一道一道走 `session/request_permission`：每个选项一项，加一项「跳过」；没有选项的题、多选的多出来的那几项收不到。

use serde_json::{Value, json};

use miyu_kernel::event::Question;

use super::forms;
use super::server::Server;
use super::session::waiting_key;
use super::tool;

/// 问了编辑器、还没等到回答的一件。
pub(crate) struct Ask {
    /// 哪个会话。
    session: String,
    /// 那一次调用。
    call: String,
    /// 脚本里调的那一件的序号；直接调用的没有。
    nested: Option<u32>,
    /// 问的是什么。
    what: What,
}

/// 问的是什么。
enum What {
    /// 工具确认：选项编号到 `session.answer` 的 `decision`。
    Approval(Vec<(String, &'static str)>),
    /// 提问，一道一道问：题目、答好了的几道、这一次问的是第几道（条目编号，问下一道时要）。
    Question {
        /// 这一组题。
        questions: Vec<Question>,
        /// 答好了的。
        answers: Vec<Value>,
        /// 条目编号。
        entry: String,
    },
    /// 提问，整组一张表单。
    Form(Vec<Question>),
}

impl Ask {
    /// 问的是哪个会话的。
    pub(crate) fn session(&self) -> &str {
        &self.session
    }
}

impl Server {
    /// 会话 `session` 在等这几件：没问过编辑器的问一次。
    pub(crate) async fn ask(&mut self, session: &str, waiting: Vec<miyu_view::Waiting>) {
        for wait in waiting {
            let key = waiting_key(&wait);
            let Some(open) = self.sessions.get_mut(session) else {
                return;
            };
            if !open.asked.insert(key) {
                continue;
            }
            let entry = wait.entry.to_string();
            let Some(step) = open.step(&entry).cloned() else {
                continue;
            };
            let ask = Ask {
                session: session.to_string(),
                call: wait.call.to_string(),
                nested: wait.nested,
                what: What::Form(Vec::new()),
            };
            match wait.what {
                miyu_view::Wait::Approve => self.approval(ask, &entry, &step).await,
                miyu_view::Wait::Ask if self.client.forms => {
                    self.form(ask, &entry, step.questions).await;
                }
                miyu_view::Wait::Ask => self.question(ask, entry, step.questions, Vec::new()).await,
            }
        }
    }

    /// 问一次工具确认。
    async fn approval(&mut self, mut ask: Ask, entry: &str, step: &miyu_view::Tool) {
        let mut tool_call = json!({
            "toolCallId": entry, "title": tool::title(step), "kind": tool::kind(&step.name),
            "status": "pending",
        });
        if let Ok(input) = serde_json::from_str::<Value>(&step.args) {
            tool_call["rawInput"] = input;
        }
        let ruled = step
            .approval
            .as_ref()
            .is_some_and(|approval| approval.rule.is_some());
        let mut choices = vec![("allow_once", "once", "allow_once")];
        if ruled {
            choices.push(("allow_session", "session", "allow_always"));
        }
        choices.push(("reject", "deny", "reject_once"));
        let options: Vec<Value> = choices
            .iter()
            .map(|(id, decision, kind)| {
                json!({"optionId": id, "name": self.language.acp_decision(decision), "kind": kind})
            })
            .collect();
        ask.what = What::Approval(
            choices
                .into_iter()
                .map(|(id, decision, _)| (id.to_string(), decision))
                .collect(),
        );
        let params = json!({"sessionId": ask.session, "toolCall": tool_call, "options": options});
        let id = self
            .editor
            .request("session/request_permission", params)
            .await;
        self.asks.insert(id, ask);
    }

    /// 整组题一张表单。
    async fn form(&mut self, mut ask: Ask, entry: &str, questions: Vec<Question>) {
        let params = json!({
            "sessionId": ask.session, "toolCallId": entry, "mode": "form",
            "message": forms::message(self.language, &questions),
            "requestedSchema": forms::schema(self.language, &questions),
        });
        ask.what = What::Form(questions);
        let id = self.editor.request("elicitation/create", params).await;
        self.asks.insert(id, ask);
    }

    /// 接着问下一道还没答的题（答好了 `answers` 那几道）；没有选项的题收不到字，当没答；都问完了交回核心。
    async fn question(
        &mut self,
        mut ask: Ask,
        entry: String,
        questions: Vec<Question>,
        mut answers: Vec<Value>,
    ) {
        while let Some(question) = questions.get(answers.len()) {
            if question.options.is_empty() {
                answers.push(json!({}));
                continue;
            }
            let mut options: Vec<Value> = question
                .options
                .iter()
                .enumerate()
                .map(|(n, choice)| json!({"optionId": format!("c{n}"), "name": choice.label, "kind": "allow_once"}))
                .collect();
            options.push(json!({"optionId": "skip", "name": self.language.acp_skip(), "kind": "reject_once"}));
            let tool_call = json!({
                "toolCallId": entry, "title": question.question, "kind": "other", "status": "pending",
            });
            let params =
                json!({"sessionId": ask.session, "toolCall": tool_call, "options": options});
            ask.what = What::Question {
                questions,
                answers,
                entry,
            };
            let id = self
                .editor
                .request("session/request_permission", params)
                .await;
            self.asks.insert(id, ask);
            return;
        }
        self.answer_questions(&ask, &questions, answers).await;
    }

    /// 编辑器回了这边问的一件（`message` 是整条回应）。认不得的编号不管。
    pub(crate) async fn answered(&mut self, id: &Value, message: &Value) {
        let Some(mut ask) = id.as_u64().and_then(|id| self.asks.remove(&id)) else {
            return;
        };
        let result = &message["result"];
        let picked = match result["outcome"]["outcome"].as_str() {
            Some("selected") => result["outcome"]["optionId"].as_str(),
            _ => None,
        };
        match std::mem::replace(&mut ask.what, What::Form(Vec::new())) {
            What::Approval(choices) => {
                let decision = picked
                    .and_then(|picked| choices.iter().find(|(id, _)| id == picked))
                    .map_or("deny", |(_, decision)| decision);
                let mut params =
                    json!({"session": ask.session, "call": ask.call, "decision": decision});
                if let Some(nested) = ask.nested {
                    params["nested"] = json!(nested);
                }
                let _answered = self.rpc.call("session.answer", params).await;
            }
            What::Question {
                questions,
                mut answers,
                entry,
            } => {
                let index = answers.len();
                let label = picked
                    .and_then(|picked| picked.strip_prefix('c'))
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| questions.get(index)?.options.get(n))
                    .map(|choice| choice.label.clone());
                answers.push(match label {
                    Some(label) => json!({"picked": [label]}),
                    None => json!({}),
                });
                self.question(ask, entry, questions, answers).await;
            }
            What::Form(questions) => {
                let answers = match result["action"].as_str() {
                    Some("accept") => forms::answers(&questions, &result["content"]),
                    _ => forms::unanswered(&questions),
                };
                self.answer_questions(&ask, &questions, answers).await;
            }
        }
    }

    /// 一组题的回答交回核心；对不上题目被拒的，照每道都没答再交一次（她拿到的是「没人回答」）。
    async fn answer_questions(&mut self, ask: &Ask, questions: &[Question], answers: Vec<Value>) {
        let mut params = json!({"session": ask.session, "call": ask.call, "answers": answers});
        if let Some(nested) = ask.nested {
            params["nested"] = json!(nested);
        }
        let reply = super::prompt::answered(self.rpc.call("session.answer", params.clone()).await);
        if reply.is_err_and(|error| error["data"]["reason"] == "bad_answer") {
            params["answers"] = json!(forms::unanswered(questions));
            let _answered = self.rpc.call("session.answer", params).await;
        }
    }
}

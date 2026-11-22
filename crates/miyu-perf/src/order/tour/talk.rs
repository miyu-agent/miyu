//! 逛一圈里说话的几步（施工 LW-14，`order/tour.rs` 的 [`super::Act`]）：说一句、回一次工具调用、回 500 以后重试、说话途中停下。
//! 每一步都要假模型收到这一轮的主请求、屏幕上画出这一步的字、会话日志里这一轮结束才算做完。

use std::time::Duration;

use super::super::head::{ANSWERED, Turns, main_request, send};
use super::super::terminal::Terminal;
use super::STEP_PATIENCE;
use crate::fake::Fake;
use crate::fake::script::{Script, ToolCall};

/// 停下的那一轮：回答切成几块、块和块之间隔多久。合起来二十秒上下，模拟器里慢的那一段也来得及在中途停下。
const PIECES: usize = 400;
const GAP: Duration = Duration::from_millis(50);

/// 回答的头几个字（[`crate::fake::filler`] 的开头）：屏幕上出现了说明回答开始往外画了。
const STARTED: &str = "量尺";

/// 按一下 Esc 以后终端界面的提示、按第二下以后这一轮底下的那一行（终端界面 `ff3834fa` 的字）。
const ESC_AGAIN: &str = "再按一次 Esc 中断";
const STOPPED: &str = "已中断";

/// 重试时转圈那一行写的字。
const RETRYING: &str = "重试";

const ENTER: &[u8] = b"\r";
const ESC: &[u8] = b"\x1b";

/// 说话的一步要的几样：伪终端、假模型、数回合的，报错时说是哪一步（`what`），这一步的记号（`marker`）。
pub(super) struct Talk<'a, 'b> {
    pub(super) terminal: &'a mut Terminal,
    pub(super) fake: &'a mut Fake,
    pub(super) turns: &'a Turns<'b>,
    pub(super) what: &'a str,
    pub(super) marker: &'a str,
}

impl Talk<'_, '_> {
    /// 说一句，等假模型收到这一轮的主请求、屏幕上画出回答、这一轮结束。
    ///
    /// # Errors
    ///
    /// 哪一样等不到。
    pub(super) async fn say(&mut self) -> Result<(), String> {
        let before = self.turns.ended()?;
        let typed = self.send().await?;
        self.request("这一轮的主请求").await?;
        self.screen(ANSWERED, typed, "画出回答").await?;
        self.turns.over(before, self.what).await
    }

    /// 说一句，假模型回一次 `tool` 的调用（参数 `args`），屏幕上画出 `shown`，`answer` 的按一下回车，等结果交回模型、这一轮
    /// 结束。
    ///
    /// # Errors
    ///
    /// 哪一样等不到；请求带着的结果对不上。
    pub(super) async fn call(
        &mut self,
        tool: &str,
        args: &str,
        shown: &str,
        answer: bool,
    ) -> Result<(), String> {
        self.fake.script(Some(Script {
            marker: self.marker.to_string(),
            calls: vec![ToolCall {
                id: format!("perf_tour_{}", self.marker.replace('-', "_")),
                name: tool.to_string(),
                arguments: args.to_string(),
            }],
        }));
        let before = self.turns.ended()?;
        let typed = self.send().await?;
        self.step(0).await?;
        self.screen(shown, typed, "画出这一次调用").await?;
        if answer {
            self.terminal.write(ENTER)?;
        }
        self.step(1).await?;
        self.turns.over(before, self.what).await?;
        self.fake.script(None);
        Ok(())
    }

    /// 说一句，假模型对这一轮的头一个请求回 500：等核心重试（再来一个请求）、屏幕上画出重试的那一行、这一轮结束。
    ///
    /// # Errors
    ///
    /// 哪一样等不到。
    pub(super) async fn retry(&mut self) -> Result<(), String> {
        self.fake.fail_once(self.marker);
        let before = self.turns.ended()?;
        let typed = self.send().await?;
        self.request("回 500 的那一个请求").await?;
        self.screen(RETRYING, typed, "画出重试").await?;
        self.request("重试的请求").await?;
        self.screen(ANSWERED, typed, "画出回答").await?;
        self.turns.over(before, self.what).await
    }

    /// 说一句，假模型慢慢地一块块回；屏幕上画出回答的头几个字以后按 Esc，等提示「再按一次」，再按一次，等屏幕上画出已中断、
    /// 这一轮结束。之后的回答照原来的一块写完。
    ///
    /// # Errors
    ///
    /// 哪一样等不到。
    pub(super) async fn stop(&mut self) -> Result<(), String> {
        self.fake.split_into(PIECES);
        self.fake.pace(Some(GAP));
        let stopped = self.stopped().await;
        self.fake.split_into(1);
        self.fake.pace(None);
        stopped
    }

    async fn stopped(&mut self) -> Result<(), String> {
        let before = self.turns.ended()?;
        let typed = self.send().await?;
        self.request("这一轮的主请求").await?;
        self.screen(STARTED, typed, "画出回答的头几个字").await?;
        let mark = self.terminal.mark();
        self.terminal.write(ESC)?;
        self.screen(ESC_AGAIN, mark, "按一下 Esc 以后").await?;
        let mark = self.terminal.mark();
        self.terminal.write(ESC)?;
        self.screen(STOPPED, mark, "按第二下 Esc 以后").await?;
        self.turns.over(before, self.what).await
    }

    /// 打这一步的一句话，等它出现在输入框里再回车：交回回车以前屏幕记到哪。
    async fn send(&mut self) -> Result<usize, String> {
        send(
            self.terminal,
            &format!("{}: keep going.", self.marker),
            self.marker,
        )
        .await
        .map_err(|e| format!("{}：{e}", self.what))
    }

    /// 等这一轮的下一个主请求（`which` 是哪一个）。
    async fn request(&mut self, which: &str) -> Result<(), String> {
        main_request(self.fake, self.marker)
            .await
            .map(drop)
            .map_err(|e| format!("{}，等{which}：{e}", self.what))
    }

    /// 等这一轮的下一个主请求，它带着的调用结果要是 `done` 个。
    async fn step(&mut self, done: usize) -> Result<(), String> {
        let arrival = main_request(self.fake, self.marker)
            .await
            .map_err(|e| format!("{}，等第 {done} 个请求：{e}", self.what))?;
        if arrival.step != Some(done) {
            return Err(format!(
                "{}：第 {done} 个请求带着的结果对不上（{:?}）",
                self.what, arrival.step
            ));
        }
        Ok(())
    }

    /// 等屏幕上（`from` 以后）出现 `needle`，最多等 [`STEP_PATIENCE`]，`when` 说是在等什么。
    async fn screen(&mut self, needle: &str, from: usize, when: &str) -> Result<(), String> {
        self.terminal
            .wait_for(
                needle,
                from,
                STEP_PATIENCE,
                &format!("{}，{when}", self.what),
            )
            .await
    }
}

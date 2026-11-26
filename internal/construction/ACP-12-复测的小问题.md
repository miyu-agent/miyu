## 施工单 ACP-12：Zed 复测撞到的四处小问题

设计 73 第四节第 4 条、第五节第 3、4、5 条、第八节。在真 Zed 里复测 ACP-10、ACP-11 时又看到四处（2026-11-26），两处改、两处查明不改。

### 为什么

1. **人自己 `miyu core stop`，Zed 却说「核心重启了，这一轮没有做完」**（低）：核心不论怎么停（人 `miyu core stop`、收到停的信号、升级换程序、
   数据根没了），停会话时都送进「要重启了」（`miyu-session` 的 `actor/stop.rs`），内核把正在进行的那一轮收成 `restarted`
   （`miyu-kernel` 的 `session/restart.rs`），再起来时接着干。核心停的那一刻不知道之后还起不起来，两种分不开。ACP-11 照「重启」写的那一句，
   人自己停的时候不对。
2. **驱动问人的确认卡片标题是「准备Bash (ACP 智能体)」，看不到命令**（中）：标题是视图拼的，`miyu acp` 原样转给 Zed（`tool::title`：显示名接
   ` (<via>)`）。`acp` 包在对方要权限时先开一块供应商侧工具、交参数，结果还要交在这一块上，所以不收块就请核心问人（`driver.approve`）。视图
   只在块收了（`end`）或者下一块开始时才当参数收全了，于是这一步一直是准备中，标题照 `view/prepare` 写成「准备Bash」（中文的字和 `Bash`
   之间没有空格）。命令原文包交上来了，核心写在确认的 `detail.command` 里（ACP-8），终端界面、网页照它画；`miyu acp` 的卡片内容只认 Miyu 自己的
   `shell` 的参数，没有用它，人只能点开 Raw Input。
3. **模型选择器里「ACP 智能体」分组出现两次**（低，不改）：`miyu acp` 照 `model.list` 一家供应商给一组（`options.rs`），核心照配置里的供应商
   一家只列一次，简称、全名都在 `acp` 这一组里。Zed 1.30.2 搜索时把所有项的结果照匹配分数排（名字匹配的在前，分数一样的照原来的先后，
   `agent_ui/src/model_search.rs`），再顺着排好的结果、前后两项的组不一样就写一次组名（`config_options.rs`）。搜 `haiku` 时「Claude Haiku 4.5」
   分数最高，magpie 的两项次之，`claude-acp/haiku` 排在后面，「ACP 智能体」就写了两次。不搜的时候只有一组。
4. **Claude 适配器每一轮都新开一个会话**（不改）：对着复测沙箱的核心日志和包的日志逐次对下来，同一个 Miyu 会话接着说的一轮没有新的
   `[session/create]`，接着用上一轮的 agent 会话（例如会话 `…c921c27b7572` 第 2、3、4 轮都在 `c57c333a` 上，`call d17`、`d19` 记着
   `#1`、`#2`）。每一轮看到的那一次新会话是第一轮说完以后起标题的辅助请求：它的 system、消息和主对话不一样，照设计 73 第四节第 4 条的指纹
   前缀对不上，新开一个；扩展被核心重启（`extension exited`）以后的那一轮也新开，这两种都是设计如此。

### 改了什么

- `miyu-cli/src/language/acp.rs`：`restarted` 的那一句改成 `核心停了，这一轮没有做完`、`the core stopped before this turn finished`。
- `miyu-view/src/projector/hosted.rs`：驱动在一块供应商侧工具上问人时（`driver_asked`；读回日志时请求先到、块后开的那一路同样），还在写参数
  的当参数收全了：换成在跑、标题照参数写（`hosted_ready`）。所有的头看到的都不再是「准备Bash」。
- `miyu-cli/src/acp/tool.rs`：工具卡片的内容（工具步骤和确认共用）在供应商侧工具带着照跑命令写的确认（`approval.detail.command`）时，先写一段
  `$ <命令原文>`，同 Miyu 自己的 `shell`。包问人的那种（没有命令）、没问过的不写。
- 文档：蓝图 `cli/acp.md`（一轮怎么翻的 `content`、给人看的字、测试表）、`view.md`（供应商侧工具的状态、测试表），设计 73 第七、八节，
  手册 `guide/editors.md`（中英）里的例句。协议没变，协议参考不用改。

### 定下的

- 第一处不去分人停和重启：要分就得让「怎么停的」进内核的收尾原因或者另记一条，是改事件模型；人看的只是这一轮没做完，一句两种都对的话就够。
- 第二处改在视图里，不改 `acp` 包：包收块就交不了结果，结果在下一块开始以后才到的路视图本来就认，但包得另记「这一块收了、结果还没交」；
  驱动问的就是这一块的参数，视图当场认参数收全更直接，也让终端界面、网页一起不再写准备中。中文模板 `准备{name}` 碰上英文的工具名没有空格，
  这一次不改（只在参数还在写的那一下出现）。
- 第三、四处不改，理由写在设计 73 第八节。

### 怎么验

- `crates/miyu-cli/src/acp/view/tests.rs` 的 `a_turn_cut_off_by_a_restart_answers_with_words_for_people`、`crates/miyu-cli/tests/acp/relink.rs`
  的 `after_a_restart_the_running_turn_fails_and_the_next_requests_go_on`：话是 `核心停了，这一轮没有做完`。修之前红：还是「核心重启了」。
- `crates/miyu-view/tests/hosted_stream.rs` 多一条 `a_block_the_driver_asks_about_has_its_arguments_written`：`Bash` 的块开着、参数交了，驱动在
  它上面问人：带着确认、在跑、标题是 `Bash`、`via` 照旧。修之前红：还是 `Preparing`。
- `crates/miyu-cli/src/acp/tool/tests.rs` 多一条 `a_hosted_command_the_driver_asks_about_shows_its_text_like_a_shell`：确认的 `detail` 带
  `command` 的，卡片内容是 `$ touch a.txt`；包问人的、没问过的没有内容。修之前红：没有内容。
- 门禁。

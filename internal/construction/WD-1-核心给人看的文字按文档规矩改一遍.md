## 施工单 WD-1：核心给人看的文字按文档的规矩改一遍

状态：做完，等验收（分支 `step/WD-1`）。

### 为什么

项目主人 2026-11-19 决定了两件事：

1. 中文界面上「key」和「密钥」混用，统一写成「密钥」。命令行的参数名和配置里的键名仍然是 `key`。
2. 核心给人看的文字按 `AGENTS.md`「文档的规矩」检查一遍：保持简短；去掉省略到看不懂的短句（例如设置项说明里的「不写的照……」）、「她」和内部的说法。给模型看的文字（提示词、工具说明）不在范围内。

网页界面和终端界面已经各自做了同样的修改，所以 `heads.toml` 也要钉到它们修改之后的提交。

### 改成什么样

**界面的提交**（单独一个提交）：`heads.toml` 的 `[webui]` 钉到 `f8db264070ae67ef6e64d1194863635c6d3b5d2f`，`[tui]` 钉到 `886772fa8f1839123c1e8a0ac457673e4bc1ef35`。这两个提交包含界面那边统一成「密钥」和改写用词的修改，在各自仓库的 CI 上全部通过。

**核心的文字**：

- `resources/core/human/zh.json`：修改 188 项。
  - 指 API 密钥的「key」改成「密钥」，一共 12 项，例如「要 key」改成「需要密钥」，「API Key 和地址」改成「API 密钥和地址」。有两句说的是配置里的键名本身（软件源的 `url` 和 `key`），保持原样。
  - 省略了条件的短句改成完整的句子，例如「配了就用你自己的 Exa 额度，不配照免费额度。」改成「填写后使用你自己的 Exa 额度，不填写时使用免费额度。」
  - `AGENTS.md` 点名的口语词换成书面的说法：「别的」改成「其他」，「跑」改成「运行」，「起不来」改成「无法启动」，「印」改成「输出」，「照旧」改成「仍然」或者直接说明行为。
  - 内部的说法换掉：「头连不上核心时会自己拉起」改成「界面连不上核心时会自动启动核心」。
- `resources/core/human/en.json`、`ja.json`：各改一句，把 `core.idle_exit` 说明里的 head、ヘッド 换成 interface、画面。其他英文和日文的句子没有修改，因为这次只改中文的写法，句子的意思没有变化。
- `crates/miyu-cli/src/language/` 等命令行的中文字符串：修改 20 句，改法同上，例如「核心还在跑，但连不上它」改成「核心还在运行，但无法连接」。命令行帮助页（`help/zh/*.txt`）已经符合规矩，没有修改。
- `crates/miyu-ipc/src/error.rs`：连接和启动核心出错时的 13 句中文，和字表里 `start/*` 的新说法保持一致，例如「核心起不来：<原因>」改成「核心无法启动：<原因>」。

**跟着修改的**：

- 蓝图 `config.md`「给人看的字」增加「用词的规矩」一段，并把表里引用的说明换成新的文字；`ipc.md`、`cli/pkg.md`、`cli/rescue.md`、`cli/check.md`、`packages.md` 等页里引用的原话一起更新。
- 配置的两份样本（`internal/designs/samples/config/` 下的中文参考文件和中文 Schema）重新生成。
- 手册 `docs/zh/guide/` 里引用程序原话的地方（故障排查、数据、软件包、联网服务等页）换成新的文字；`docs/authors/` 里引用确认屏文字的两处同样更新。
- 断言这些文字的测试改成新的文字。

### 不做什么

- 不修改给模型看的文字。
- 不修改已经简短清楚的句子，例如「已取消」「强制推送……前先问。」。
- 不修改施工单、设计文档里引用的旧文字，因为它们记录的是当时的样子。
- 「不支持该系统」这个标签没有修改：网页界面的软件包列表显示同一个标签，两边需要一起修改。

### 验收

- 修改之前守卫测试是红的。
- 门禁、Windows 目标的 clippy、三个平台的 CI 全部通过。
- 实测：在沙箱数据根里运行命令行，查看新的文字。

### 做下来的

- 改前红：把中文字表换回修改之前的版本运行 `human_wording::chinese_words_say_miyao_and_never_she`，测试失败。按同样的规则统计，修改之前的字表里有 12 句出现指密钥的「key」，修改之后只剩保留的两句键名。字表里原本没有「她」「他」，这一项只是防止以后写回去。
- 全量测试：第一趟有 36 条测试失败，全部是断言旧文字的测试；把断言换成新文字之后重新运行，再修改最后两条（`miyu` 的 `rescue::an_unreadable_config_fills_nothing_and_still_launches`、`miyu-view` 的 `markdown_shell::a_running_command_says_so_and_an_odd_ending_says_why_first`），其余全部通过。门禁的结果以交回时的说明为准。
- 样本：用 `MIYU_SAMPLE_WRITE=1` 重新生成了 `internal/designs/samples/config/` 下的四份文件。英文的两份也有变化，因为英文字表改了 `core.idle_exit` 的那一句。
- 实测（2026-11-19，`MIYU_HOME` 和 `HOME` 都在沙箱里，没有使用模型，用的是这一步编出来的程序）：
  - 数据根里已有内容但没有标记时，`miyu check` 输出 `… 里有其他内容，无法确认它是 Miyu 的数据根，没有修改它。如果它是 0.7 版的数据目录，先 mv … …-0.7，再 miyu pkg install migrate、miyu migrate …-0.7；如果不是，把 MIYU_HOME 设成一个空目录`。
  - 在空的数据根里 `miyu core start` 之后，写入下面「怎么实测验收」里的配置，再加一家引用环境变量的供应商和一个指向不存在的供应商的 `models.chat`。`miyu check` 输出 `providers.acme.key 引用的密钥 acme 还没有设置。`、`providers.far.key 引用的环境变量 WD1_NO_SUCH_VAR 在核心启动时没有设置。`、`models.chat 指向的供应商 nosuch 没有配置。`。
  - `miyu config explain providers.acme.key` 的第一行以 `密钥（providers.acme.key）：写成 { secret = "名字" }…` 开头。数据根里生成的 `var/cache/config/reference.toml` 使用的也是新的说明。
  - `miyu core stop` 之后，`miyu core status` 输出 `核心没有在运行`。
  - 把 `MIYU_RESOURCES` 指到不存在的目录再 `miyu core start`，输出 `miyu: 核心无法启动（resources_missing）：<路径>`。此时命令行读不到字表，输出的是 `miyu-ipc` 里的那一句。
  - 没有实测的部分：网页界面和终端界面的设置页。这两个界面在各自的仓库里，这一步没有编译它们；设置页显示的名字和说明来自同一份字表。

### 怎么实测验收

1. 运行 `miyu core stop`，确认之后的命令由新版本的核心回答。
2. 把 `MIYU_HOME` 和 `HOME` 都指到试验用的空目录，在 `etc/config.toml` 里写入：

   ```toml
   [providers.acme]
   driver = "openai-chat"
   base_url = "http://127.0.0.1:9/v1"
   key = { secret = "acme" }
   ```

3. 运行 `miyu check`，警告应该是 `providers.acme.key 引用的密钥 acme 还没有设置`（修改之前是「还没设」）。
4. 运行 `miyu config explain providers.acme.key` 或者打开网页界面的设置页，这一项的名字应该是 `密钥`（修改之前是 `key`）；`联网服务` 下拉里没有配置密钥的服务后面显示 `需要密钥`。

## RT-1 session.respond 带接在最末尾的一句（tail）

状态：施工完，待 core1 审（2026-11-07）。项目主人 11-07 要的：im 在群聊每一轮末尾带一句固定的提示，实测只有放在最新消息后面才有效。
形状 core1 11-07 定；做之前问的四点 core1 11-07 照推荐定（见「定了的」第 4 到 7 条）。

### 定了的

1. `tail` 是字符串，最多 2 KiB（超了 `bad_params`，`data.field` 是 `tail`），空串当没写；只在 respond 开出来的那一轮有效。
2. 这一轮每次请求都把它当最后一块文本，放在整份 messages 的最末尾：第一步紧跟触发的几条，带工具的后面几步接在最新的工具结果后面。
   不进有效历史、视图流、导出、压缩，下一轮不渲染。
3. 落盘：记在这一轮开头那条 `turn.started` 上，只为重启以后接着跑的那一轮还带得上；渲染历史不读它。
4. **放在哪（A）**：`Request` 加 `tail`，驱动编完消息以后接在最后；指纹、组装器、messages 不动；anthropic 最后那个缓存打点打在它前面
   那一块上；规范字节带它才写，老请求的字节和哈希不变。理由：放进 messages 的话每一步最后那条都变，`first_difference` 每步都不为空、
   `cache_breaks` 会算成每步断一次；anthropic 第 4 处打点落在它上面，下一步接不上。
5. **接着写**：openai-chat 前缀续写的那一次请求不带，别的照带。
6. **正在跑的一轮来的 respond**：带了的换掉这一轮的，记在那条 `turn.joined` 上，重启接着跑照样带；没带的不清掉原来的。
7. **原始包导出**照日志原样留，不过滤。
8. 协议参考照 `session.respond` 所在的 `venues.md`、`venues.en.md` 写（core1 原话写的是 `sessions.md`，那里没有这个方法）。

### 做了什么

- **内核**（`miyu-kernel`）：
  - `turn.started`、`turn.joined` 加 `tail`（没有的不写，老日志一个字节不变）；`Command::Respond` 加 `tail`。
  - `Turn` 记这一轮的那一句：`open_turn_on` 收 `tail`、写进 `turn.started`；`respond` 开的、不写 `to` 照事实开的都带；并进正在跑的一轮的
    带了才换。
  - `ask()` 算完指纹才把它放进 `Request.tail`：工具第二步、下一轮照旧是前缀延伸。
  - 这一轮没再请求就结束、由并进来的几条（`Waker::Joined` 的回报）接着开的那一轮，带上这一轮的那一句（`queue.rs` 的 `finish_turn`）：
    接着开的就是为了答它们。由排着队的消息、后台命令的回报接着开的不带。
  - 载入（`load.rs`）：`Replay` 照 `turn.started`、带了的 `turn.joined` 记下最后那一句，放进 `Ended`；被有计划的重启打断、接着干的那一轮
    照它写进新的 `turn.started`、设到回合上。
  - `Request.tail`（`request.rs`）：`skip_serializing_if`，不进 `fingerprint`。
  - 测试工具 `testkit/venue.rs`：`Stage::overhear`、`Stage::respond`。
- **驱动**（`miyu-drivers`）：新的 `tail.rs` 的 `tailed`：有那一句、不跳过的，照一份请求接上（最后是 user 的并进去做最后一块，别的另起
  一条 user）。三家整份编的（`fragments`）、接着用留着的历史编的（`fragments_memo`）都先过它，发出去的字节一样；接上的那一条和平常的
  消息一样进备忘录，下一次照「从第一条不一样的起重编」重编那一条。openai-chat 前缀续写的那一次跳过。anthropic 的 `marks::place`
  多一个 `tail`：第 4 处打在它前面那一块，单独成了一条的打在上一条末尾。
- **端点**（`miyu-endpoint`）：`RespondParams.tail` 先收成任意的值自己查（写错的要报 `data.field`）：`null`、空串当没写，不是字符串、
  超过 2 KiB 的 `Refusal::bad_field("tail")`。
- **文档**：参考 `venues.md`、`venues.en.md` 的 `session.respond`；蓝图 `kernel/request.md`「组装」第 8 条、`kernel/events-bodies.md`
  的 `turn.started`、`turn.joined`、`drivers/anthropic.md` 打点第 4 条、`drivers/openai-chat.md` 接着写、`protocol.md` 方法表。核心
  不加给模型看的字，登记簿不动。

### 测试

- 内核场景 `session/tests/scenario/respond_tail.rs`（真内核、替身）：
  - 带了的这一轮两步都带，下一轮不带；messages 里没有它；`turn.started` 记着；第二步、下一轮的 `model.called` 没有 `first_difference`；
  - 同一份剧本不带它跑：每次请求去掉它，规范字节、指纹一字不差；带了的规范字节最后多 `"tail"` 那一格，不带的没有这一格；
  - 正在跑的一轮来 respond：带了的换掉、没带的不动（三次请求 `一`、`二`、`二`），两条 `turn.joined` 一条带一条不带；接着开的那几轮的
    `turn.started` 记着那一句；
  - 被有计划的重启打断：接着干的那一轮照样带，写进新的 `turn.started`；并进来时带的，重启以后照样带。
- 请求形状探针 `miyu-assemble/tests/probe_tail.rs`（真内核、真组装器、三家驱动）：
  - 去掉那一句，五条性质照旧成立，和不带它跑的逐字节一样（统一的请求、openai-chat、去掉打点的 anthropic、responses 四层）；
  - 每次请求带和不带只差插进去的那一段，就是那一句；
  - 下一次请求（工具第二步、第二轮）把上一次那一句前面的字节一个不少地接着用（只差那一句和它前面接的分隔）；anthropic 上一次最后那个
    打点在那一句前面，下一次在同一处照样打了。
  - 现有的探针存档一个字节没变（`probe` 全过，没有重写存档）。
- 驱动 `tests/tail.rs`：三家带和不带逐字节比：openai-chat 并进最后那条 user 是 `\n` 加它、工具结果后面另起 `{"role":"user",...}`；前缀
  续写的那一次一字不差；responses 同样；anthropic 多一个 text 块、最后那个打点打在它前面（到那里和不带的一字不差）、那一句后面不打点；
  工具第二步只丢那一句，上一步最后那个打点下一步在同一处也打了。`src/tail/tests.rs` 查接在哪。备忘录的随机对照（`tests/memo.rs`）
  多一种改动 `Tail`：接上、换一句、拿掉，每一次整份编的和接着用的照旧逐字节一样。
- 端点 `tests/respond.rs`：带了的记在 `turn.started` 上、发给模型的请求带着它，空串当没写；不是字符串、数组、2049 字节的 `bad_params`、
  `data.field` 是 `tail`、什么都不记，正好 2 KiB 的收。

### 变异

手写 12 个，抓住 11 个：`ask()` 不放、并进来的不管带没带都换、都不换、接着开的那一轮不带、载入不认 `turn.joined` 带的、重启接着干的
不带、驱动从不并进最后那条 user、anthropic 打点不让开那一句、端点上限多一个字节、端点收了字却丢掉、规范字节没有的也写 `"tail":null`
（现成的探针存档当场红）。

**没抓住 1 个**：openai-chat 前缀续写的那一次照样接上（去掉 `skip`）。续写时最后那条 user 只有被打断的那一句，那一句并进的正是它，
续写本来就整条不发，跟着一起没了：整份编的、接着用留着的两条路都这样，字节一样，是等价的。`skip` 留着把意思写明白，免得以后续写的
写法变了漏掉这一条。

### 坑

- 一开始只让并进来的 respond 换掉回合上的那一句，测试红了：每次回复都没再调工具时，这一轮没再请求就结束，并进来的几条另开一轮，
  新开的回合上是空的。改成由 `Waker::Joined` 接着开的那一轮带上这一轮的那一句。
- anthropic 的打点本来就随轮挪（第 3 处跟着最后那条 assistant 走），比前缀要去掉打点（现成的探针也这样）；接得上没有另外查打点的位置。

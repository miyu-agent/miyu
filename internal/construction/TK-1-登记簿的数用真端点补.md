## 施工 TK-1：HT-2 四段字的 token 数用真端点补

状态：已完成（2026-11-17）。发版前核对的一批，核心主会话派。只改登记簿（`internal/designs/26-提示词.md` 第十节）和照它重出的 `internal/blueprint/prompts.md`，给模型看的字一个字节没变。

### 为什么

HT-2 的四段字（`resources/core/drivers/hosted-tool.txt`、`hosted-tool-failed.txt`、`hosted-tool-open.txt`、`hosted-tool-cut.txt`）登记时没有真端点，token 数照 o200k 本机估、标着「约」。登记簿别的行照真端点量，这四行补上。

### 怎么量

- 端点：中转站的 `relay/deepseek-v4-1-flash-261017`（OpenAI 兼容的 chat completions），量的是 DeepSeek V4.1 Flash 的分词。地址、key 只在命令的环境变量里，不进仓库。
- 量法照登记簿已有的「真端点量」：一条 user 消息 `hi` 量一次 `prompt_tokens`，同一条消息换成 `hi`、一个换行、那段字再量一次，相减。每样量两次，两次一样。
- 字段：外壳照字段空着、带行尾换行；整块照 HT-2 登记时的例子（`web_search`、参数 `{"query":"rust 1.97 release date"}`、结果 `Rust 1.97 was released on 2026-07-07.`）填好、去掉末尾的换行（同驱动交出去的样子）。

### 量到的

| 文件 | 外壳 | 整块 | o200k 本机估（HT-2） |
|---|---|---|---|
| `hosted-tool.txt` | 21 | 51 | 18、47 |
| `hosted-tool-failed.txt` | 21 | 51 | 18、47 |
| `hosted-tool-open.txt` | 15 | 28 | 12、25 |
| `hosted-tool-cut.txt` | 6 | — | 5 |

`hi` 一条是 31（两次一样）。真端点的数含接上去的那个换行，比 o200k 估的多 1 到 4。

### 验收

1. 登记簿四行换成真端点的数、写明哪个模型的分词、留着 o200k 估的数。
2. `cargo xtask prompts` 重出 `prompts.md`，门禁「文档」一项对得上。

## `miyu call`

### 是什么

在 AI 跑的命令里调这个会话的工具：脚本里把工具当函数用，结果交给脚本，不进她的上下文（设计 `39-脚本调工具.md`，施工 M-3）。连的是这条命令自己的端点（`tools/shell.md`「脚本调工具」），不连核心的协议、不读本机令牌：没有端点的一律退出码 2，不退回去经协议调（MC11）。

Windows 的端点在施工 M-3 下；那之前 Windows 上的 `miyu call` 照「只能在 Miyu 跑的命令里用」退出码 2。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 第一个词是 `call` 的，不读清单、不经 clap，当场交给 `miyu_cli::call` |
| `crates/miyu-cli/src/call.rs` | 写法、发请求、照回答印和退出 |
| `crates/miyu-cli/src/help/{zh,en}/call.txt` | 帮助页 |
| `resources/software/basesystem/shell/call/{outside,ended,usage,bad-json}.txt` | 命令行这头说的几句，编进程序；和端点那头的放在一起、一起登记（`26-提示词.md` 第十节） |

### 对外的样子

```text
miyu call <工具> --<参数> <值>...
miyu call <工具> --json '<JSON 对象>'
miyu call <工具> --json -              （参数从标准输入读）
miyu call --schema <工具>...           （一次查几件）
miyu call <工具> --schema
```

- 具名参数：`--query "niri 输入法"`、`--limit=8`；值一律当字符串发，核心照工具的格式修成数、布尔（`repair`）；同一个参数写几次的是数组。`--json` 交一个 JSON 对象，`-` 从标准输入读；具名的盖过它里面同名的。
- `--schema` 印这件的说明和参数格式（`tools/shell.md`「脚本调工具」）；后面跟几个名字的一件一件印，有一件不成的照印、退出码取最后不成的那件的。结果的样子（`Result:`）等施工 M-4 和说明一起定。
- `-h`、`--help` 印帮助页。
- 退出码照 `22-命令行.md` O1：

| 码 | 意思 | 标准错误 |
|---|---|---|
| 0 | 成了，结果在标准输出：有结构化结果的一行 JSON（中文不转义），别的照原样的字，都以换行结尾 | 交回的图没给脚本的说一句 |
| 1 | 工具执行了但出错，或者被拒 | 工具自己的原话；拒绝照直接调用的原话 |
| 2 | 用法不对，没执行：写法不对、不认识、调不了、参数不是对象、不在 Miyu 跑的命令里、不是这条命令的进程、版本对不上 | 一句，能指个改法的跟一个 |
| 3 | 被打断，或所属的命令已经结束 | |
| 4 | 要人确认，没做 | 叫她直接调那一件 |

### 怎么走

1. 写法照上面拆成一个或几个请求；写错的印 `usage.txt`、退出码 2，`--json` 不是对象的印 `bad-json.txt`。
2. 读 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN`：没有的照 `outside.txt` 退出码 2。
3. 连套接字：不在、没人听的是端点关了，照 `ended.txt` 退出码 3；别的连不上照 `outside.txt` 退出码 2。
4. 一次连接一个请求：4 字节大端长度加 JSON（`{v, op, tool, args, token}`，`op` 是 `call` 或 `schema`，`args` 是参数对象的原文）；收回 `{code, stdout, stderr}`，照原样印、照 `code` 退出。读写断了的照 `ended.txt` 退出码 3。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/call/tests.rs` | 具名参数是字、写几次成数组；`--json` 要对象、具名的盖过它；`--schema` 两种写法、一次几件；写错的说怎么写；没有那两个变量的退出码 2 |
| `crates/miyu/tests/call.rs` | 真核心、真 `shell` 里调（见 `tools/shell.md`「守着它的」） |

### 出处

设计 `39-脚本调工具.md` 第五节第 8 条、第六节；施工单 `M-3-通道和命令.md`。原型 `step/M-0-脚本调工具原型` 的 `crates/miyu-cli/src/call.rs`。

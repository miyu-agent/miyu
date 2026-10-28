# 配置、密钥、模型：`config.*`、`secret.*`、`provider.*`、`model.*`、`usage.query`

英文版：[config.en.md](config.en.md)。

| 方法 | 做什么 |
|---|---|
| [`config.schema`](#configschema) | 配置清单：每一项的名字、说明、类型、能写在哪 |
| [`config.get`](#configget) | 最终值和来源，现在的全部问题 |
| [`config.set`](#configset) | 改一项或几项、恢复默认，或者整份换掉 |
| [`config.check`](#configcheck) | 把一段字当成一层配置查，不生效 |
| [`config.trust`](#configtrust) | 信任、不信任一份项目配置 |
| [`secret.set`、`secret.delete`、`secret.list`](#secretsetsecretdeletesecretlist) | 存、删密钥，列密钥的名字 |
| [`model.list`](#modellist) | 配好的供应商和模型、池、在用的目录 |
| [`provider.detect`](#providerdetect) | 找现成的：环境里设了的 key、本机跑着的模型服务 |
| [`provider.catalog`](#providercatalog) | 搜供应商目录 |
| [`provider.test`](#providertest) | 试一家供应商通不通 |
| [`provider.rename`](#providerrename) | 改一家供应商的编号 |
| [`model.call`](#modelcall) | 叫一次模型，不进任何会话 |
| [`model.embed`](#modelembed) | 算一批字的向量 |
| [`usage.query`](#usagequery) | 查用量和金额 |

## 配置的三层

配置从下往上叠：出厂默认值 → 系统配置（`config.toml`，你的设置都写在这里）→ 项目配置（仓库里的 `.miyu/config.toml`，要信任过才算、只能收紧）。每一项能写在哪几层写在清单里（`layers`）。密钥另放 `secrets.toml`，配置里写 `{ secret = "<名字>" }` 引用它，或者写 `{ env = "<环境变量>" }`。

**问题**：读、查、改配置时发现的每一处写成一条：`level`（`error` 或 `warning`）、`code`（原因码）、`file`、`line`、`column`、`key`、`expected`、`got`、`suggest`（拼错的键最近的那个）、`using`（现在照什么用着）、`message`（整句给人看的话，照连接的语言）。没有的格不写。

**谁能调**：读的（`config.schema`、`config.get`、`config.check`、`secret.list`、`model.list`、`provider.*`、`model.call`、`model.embed`、`usage.query`）人、扩展都能调。改的（`config.trust`、`secret.set`、`secret.delete`）只给人，扩展调回 `local_only`。`config.set` 扩展只能逐项改系统配置里自己包编号开头的、不是密钥、不写成引用的项。

## `config.schema`

**参数**：`keys`（字符串数组）：只要这几项，不写是全部。

**回应**：`items`（每一项，照清单的先后）、`pages`、`groups`（设置页的页和组）。一项：

| 名字 | 说明 |
|---|---|
| `key`、`type` | 键、类型：`bool`、`option`、`int`、`float`、`text`、`duration`、`path`、`url`、`name`、`reference`、`model`、`model_or`、`english`、`secret`、`list`、`table`。键里人起的名字那一段写成占位 `<id>`、`<model>`、`<external>`、`<name>`，例如 `providers.<id>.base_url` |
| `options` | 选项：`value`、给人看的 `name`；核心查得出的带 `note`（暗色写在名字后面）、`available: false`（用不了，画成灰的） |
| `ticks` | 列表带选项的怎么画勾：`unlisted` 的勾着的是不在值里的（`sandbox.private.remove`：勾着的照旧私密，取消勾选＝加进值里；`sandbox.readonly.remove` 同理，勾着的照旧受保护）；没有的勾着的是值里的 |
| `min`、`max`、`max_chars`、`element` | 范围、长度、列表元素的类型 |
| `default` | 默认值 |
| `layers` | 能写在哪几层：`system`、`project` |
| `applies` | 什么时候生效：`now` 当场、`next_turn` 下一轮、`new_session` 以后开的会话、`restart` 重启核心、`head_start` 头下次启动 |
| `name`、`description` | 给人看的名字、说明，照连接的语言 |
| `page`、`group`、`common`、`control` | 设置页放哪、常不常用、用什么控件 |
| `suggestions` | 控件是 `combo` 的：建议值 |
| `hidden` | 设置页不画 |
| `package` | 软件包的配置项：归哪个包 |

**出错**：`unknown_config_key`：`keys` 里有清单里没有的，`data.problems` 每个一条，带最近的键名 `suggest`。

## `config.get`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `keys` | 字符串数组 | 只要这几项，不写是全部 |
| `cwd` | 字符串 | 照这个目录找项目配置、算进最终值；不写不算 |
| `all` | 布尔，默认 `false` | 每一项再列出写了它的每一层 |

**回应**

| 名字 | 说明 |
|---|---|
| `items` | 键 → `{value, origin}`：最终值和来源（`origin` 有 `layer`、`file`、`line`）。`all` 的多 `layers`：从上往下每一层 `{origin, value, used}`，没生效的带 `problem` |
| `files` | 每一层的文件：`system`、`project`（`file`、`version`；项目的多 `trusted`：`true`、`false` 或 `null` 还没问过），`secrets`（只有 `file`） |
| `problems` | 这几份文件现在的全部问题 |

```json
{"files":{"secrets":{"file":"secrets.toml"},"system":{"file":"config.toml","version":"sha256:…"}},"items":{"ui.language":{"origin":{"file":"config.toml","layer":"system","line":3},"value":"zh"}},"problems":[]}
```

**出错**：`unknown_config_key`。

## `config.set`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `layer` | `system`，可以不写 | 改哪一层：只有系统配置（写 `personal` 的 `bad_params`） |
| `changes` | 数组 | 改几项，每项见下 |
| `text` | 字符串 | 整份换成这段字 |
| `version` | 字符串或 `null` | 写了 `text` 的必写：你读到的这份文件的版本，`null` 是那时还没有这份文件 |

`changes` 和 `text` 正好写一个。`changes` 每一项：`key`（必写）；`value`（JSON 的值）、`input`（人敲的字，照这一项的类型读）、`unset: true`（恢复默认）三选一；`expect`（可不写）：这一层这一项现在应当是 `{"value": …}` 或 `{}`（没写），对不上就拒。

**回应**：`{"keys", "version"}`。`keys` 是变了的每一项：`value`（这一层现在的值，删掉的不写）、`effective`、`origin`（算上别的层以后的最终值、来源）、`applies`。一项都没变的 `keys` 是空的，什么都不写。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"config.set","params":{"changes":[{"key":"ui.language","value":"zh"}]}}
← {"id":"s1","jsonrpc":"2.0","result":{"keys":{"ui.language":{"applies":"now","effective":"zh","origin":{"file":"config.toml","layer":"system","line":3},"value":"zh"}},"version":"sha256:…"}}
```

几项一起查、一起写：有一项不对整条不收。写盘、推 `config.changed` 以后才回应。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `changes`、`text` 不是正好一个；一项里 `value`、`input`、`unset` 不是正好一个；同一个键写两次；写了 `text` 没写 `version` |
| `unknown_config_key` | 清单里没有的键，`data.problems` |
| `config_invalid` | 值不对、不能写在这一层，`data.problems` 是每一处。`sandbox.private.add` 写了家目录本身或它的上级的那一处 `code` 是 `private_too_broad`，`got` 是那一个目录；`sandbox.readonly.add` 写了家目录本身或它的上级的是 `readonly_too_broad` |
| `config_conflict` | `expect` 对不上（`data.current`）；`version` 对不上，或者一直有人在手改（`data.version`） |
| `config_file_broken` | 这份文件现在读不进来，没法只改几项，`data.problems` |
| `local_only` | 扩展改了不许它改的 |

## `config.check`

**参数**：`layer`（`system`、`project`，必写）、`text`（必写）。**回应**：`{"problems": [...]}`，没问题是空数组；问题不带 `file`。

## `config.trust`

**参数**：`cwd`（必写，照它找项目配置）、`version`（必写，人看过的那一份的版本：`config.get` 的 `files.project.version`）、`trust`（布尔，必写）。

**回应**：`{"file", "trusted"}`。只能信任人看过的那一份。

**出错**：`no_project_config`；`config_conflict`（看过以后它又变了，`data.version` 是现在的）。

## `secret.set`、`secret.delete`、`secret.list`

| 方法 | 参数 | 回应 |
|---|---|---|
| `secret.set` | `name`、`value`（都必写） | `{"replaced": <原来有没有>}` |
| `secret.delete` | `name` | `{}` |
| `secret.list` | 没有 | `{"secrets": [{"name", "set", "used_by"}…]}` |

存的是去掉前后空白的值。`secret.list` 从不交出值：列设了的和配置里引用了、还没设的，`used_by` 是引用它的配置键。

密钥只有一份：`secrets.toml`。

**出错**：`bad_params`（名字不合写法；值是空的、有控制字符、超过 16 KiB）；`unknown_secret`（删的没有）；`config_file_broken`（密钥文件读不进来）；`local_only`（扩展调了改的两个）。

## `model.list`

**参数**：`provider`（只看这一家）、`refresh`（布尔，先去拉一遍供应商的模型列表再答）、`models`（字符串数组，只要这几个 `<供应商>/<模型>`）。

**回应**

| 名字 | 说明 |
|---|---|
| `providers` | 配好的供应商，照编号排。每家：`id`、`name`（`{value, from, key}`）、`driver`、`base_url`、`key`（`{ref, set, state}`，从不交值）、`catalog`（对上了目录里哪一家）、`logo`（`{svg, tint}` 或 `null`）、`models`；用不了的另带 `problem` |
| `models` 里每个 | `model`、`ref`、`listed`（从哪几处列出：`config`、`provider`、`catalog`）、`facts`（窗口、最大输出、输入种类、能不能用工具、思考、价格、名字……每格 `value` 和来源）、`state`（`ok`、`cooling` 带 `until`/`class`、`no_key`），能出向量的带 `embedding: true` |
| `pools` | 每个池：`name`、`strategy`、`models`、`usable`、`subagent`、`description`、`fixed`（`lite`、`normal`、`flagship` 三个固定池是 `true`：永远在，配置里没写也列出来，不能删、不能改名，`subagent` 默认 `true`） |
| `side` | 旁路请求（不在对话里的那些模型请求）的登记，每条：`id`（`core/title` 起标题、`memory/organize` 整理记忆）、`tier`（要哪一档：`lite`、`normal`、`flagship`）、`name`、`summary`（照连接的语言）、`resolved`（这一刻会落到哪：档位池有成员的是池名，空的是 `session`，用会话的模型）。池里全在冷却、都出错了的，也退到会话的模型。扩展在清单 `[[side]]` 里登记的也在这里，`id` 是 `<包>/<编号>`，名字、说明照它清单里写的 |
| `uses` | `chat`、`vision` 各配的引用，没配的是 `null` |
| `catalog` | 在用的目录：`source`、`fetched`，读不了是 `null` |

**出错**：`unknown_provider`；`bad_params`（`models` 写法不对）。

## `provider.detect`

**参数**：没有。**回应**：

| 名字 | 说明 |
|---|---|
| `keys` | 核心的环境里设了的 key：`env`、`provider`、`name`、`driver`、`supported`，已经配了的带 `configured`。不交值 |
| `local` | 本机跑着的模型服务：`provider`、`name`、`base_url`、`models`，已经配了的带 `configured` |
| `looked_for` | 找了哪些环境变量的名字 |

## `provider.catalog`

**参数**：`query`（编号、名字里有这一截的）、`limit`（默认 50）、`featured`（布尔，只要常用的几家，照连接的语言挑国内外）。

**回应**：`{"providers": [...]}`，每家 `id`、`name`、`driver`、`base_url`、`env`、`doc`、`models`（几个模型）、`supported`、`local`（地址在本机、不要 key）、`logo`。能用的排前面。

## `provider.test`

**参数**：`provider`（配好了的）或 `candidate`（还没写进配置的：`driver`、`base_url`、`key`（`{secret}`、`{env}` 或 `{value}`，`value` 只用这一次、不存）、`catalog`），正好写一个；`model`（拿哪个试）。

列得出模型就算通，不发请求；列不出、写的模型不在列表里时才发一句，那时会花一点额度。

**回应**：成了 `{"ok": true, "checked", "models", "listed", "model"}`，发了一句的多 `first_token_ms`；没成 `{"ok": false, "stage", "error": {"class", "status"?, "message"}}`，`stage` 是 `list`、`request`、`config`，给一个模型名就能再试的带 `need_model: true`。

```json
{"ok":true,"checked":"list","models":["deepseek-flash","deepseek-v4-pro"],"listed":"provider","model":"deepseek-flash"}
{"ok":false,"stage":"request","error":{"class":"auth","status":401,"message":"Authentication Fails (no such user)"}}
```

**出错**：`bad_params`；`unknown_provider`。

## `provider.rename`

改一家供应商的编号，一次改完：两层配置里 `[providers.<from>]` 那张表（连同子表、密钥的引用）换成 `<to>`，`models.chat`、`models.vision`、`models.embedding`、池的成员这些 `<from>/<模型>` 换成 `<to>/<模型>`；钉着旧编号的会话下一轮起照新编号。只给人。

**参数**：`from`、`to`。`to` 照编号的写法：小写字母开头，小写字母、数字、`-`、`_`，最多 32 个。

**回应**：`{}`；改了的那几层照 [`config.changed`](pushes.md) 推。

**出错**：`bad_params`（`to` 写法不对、和 `from` 一样）；`unknown_provider`（没有 `from`）；`provider_exists`（已经有 `to`）。两层里后一层写不成的，前一层改了的不撤，照那一层的错回（`config_conflict`、`config_file_broken`），再调一次照剩下的改。

## `model.call`

叫一次模型，拿整段回答和用量。不进任何会话、不推送；会花额度，记进用量。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `purpose` | 字符串，必写 | 用途，1 到 32 个小写字母、数字、`-`，例如 `vision` |
| `messages` | 数组，必写 | `{"role": "system"|"user"|"assistant", "text", "images"?}`。`system` 至多一条、只能在最前；最后一条是 `user`；`images` 只有 `user` 能带，是 blob 的哈希：先照这个连接自己的账号找（`blob.put` 存在那里），找不到再照它替谁干活的那个人找；都没有的 `unknown_attachment` |
| `model` | 字符串 | `<供应商>/<模型>` 或 `@<池>`；不写照 `models.chat` |
| `use` | `chat` 或 `vision` | 不知道人配了哪个模型时用：照 `models.chat` 或 `models.vision`；和 `model` 至多写一个 |
| `side` | 字符串 | 扩展的旁路请求：写自己包里清单 `[[side]]` 登记的编号（不带包名）。照登记的档位从同名的池（`lite`、`normal`、`flagship`）取；池是空的、池里全在冷却或者都出错了，用会话的模型。和 `model`、`use` 不能同写；人的连接、没登记的编号 `bad_params` |
| `session` | 会话编号 | 只跟着 `side`：用不上档位池时照这个会话这一刻的模型；不写的照 `models.chat`。不是自己的会话 `session_not_found` |
| `max_tokens` | 正整数 | 最多输出多少 token |

**回应**：`{"text", "provider", "model", "usage"}`。`usage` 是 `uncached`、`cache_read`、`cache_write`、`output`，报了思考的多 `reasoning`；没报的是 `null`。在后台答，不挡这个连接别的请求。

```json
{"text":"A cat on a red sofa.","provider":"deepseek","model":"deepseek-flash","usage":{"uncached":812,"cache_read":0,"cache_write":0,"output":9}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 参数写法不对；图的 blob 不是图；`side` 不是这个扩展登记过的 |
| `unknown_attachment` | 图的 blob 这个账号没有 |
| `unknown_model` | `model` 解析不出 |
| `no_model` | 没有能用的模型（没配、key 取不到），`data.message` |
| `cooling` | 候选全在冷却，`data.message`、`data.wait_ms` |
| `model_failed` | 发了、出错了：`data.class`、`data.status`、`data.message` |

## `model.embed`

**参数**：`texts`：1 到 64 条字，每条 1 到 8000 个字。照这个账号的 `models.embedding` 算（本机的内置模型或远程的一家）。

**回应**：`{"model", "vectors"}`。`model` 是模型编号（`local:<id>` 或 `<供应商>/<模型>`）：换了模型的向量不能和以前的比。`vectors` 照先后，归一化过。

**出错**：`bad_params`；`embedding_off`（关着，或者要本机的却没装内置模型）；`embedding_failed`（有一条算不出、等不到，`data.why`）。

## `usage.query`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `from`、`until` | 时刻 | 只算这一段，左闭右开 |
| `group` | 数组 | 照这几样分组：`account`、`venue`、`model`、`day`、`session`、`purpose`、`use`（用途：`main` 主对话、`compaction` 压缩、`subagent` 子代理、`recap`、`title`、`memory`、`other`）；不写是一行总计 |
| `session` | 字符串 | 只算这个会话；`tree: true` 连它派的子会话 |
| `offset` | 时区，例如 `+09:00` | 分天照哪个时区，不写是核心所在机器的 |

**回应**：`{"rows": [...]}`。一行：分组的那几格，`requests`（请求数）、`usage`（四项加起来）、`amounts`（照币种各加各的，原样）、`unpriced`（有用量、算不出金额的次数）、`converted`（换算成 `usage.currency` 的一个数：每一笔照它那天的汇率换，`{"currency","amount"}`；取不到汇率的那几笔在它的 `rest` 里，写法同 `amounts`；一笔都没换成的不写）。人人的会话都记账；不是管理员的只查得到自己名下的。金额照 `usage.currency` 排、换算：查的人自己那一份配置的。用到了 open.er-api.com 的汇率时，回应顶上多 `rates`（来源和链接），界面要注明来源。

```json
{"rows":[{"purpose":"vision","model":"deepseek/deepseek-flash","requests":2,"usage":{"uncached":1624,"cache_read":0,"cache_write":0,"output":18},"amounts":[{"currency":"USD","amount":0.0002544}],"unpriced":0}]}
```

**出错**：`bad_params`。

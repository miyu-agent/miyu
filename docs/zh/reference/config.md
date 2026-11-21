# 配置、密钥、模型：`config.*`、`secret.*`、`provider.*`、`model.*`、`usage.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

| 方法 | 做什么 |
|---|---|
| [`config.schema`](#configschema) | 获取配置清单：每一项配置的名字、说明、类型，以及它能写在哪几层 |
| [`config.get`](#configget) | 获取每一项的最终值和来源，以及配置现在的全部问题 |
| [`config.set`](#configset) | 修改一项或几项配置、恢复默认值，或者替换整份配置文件 |
| [`config.check`](#configcheck) | 把一段文字当作一层配置来检查，检查的内容不会生效 |
| [`config.trust`](#configtrust) | 信任或者不信任一份项目配置 |
| [`secret.set`、`secret.delete`、`secret.list`](#secretsetsecretdeletesecretlist) | 存入、删除密钥，列出密钥的名字 |
| [`model.list`](#modellist) | 列出配置好的供应商和模型、模型池，以及正在使用的目录 |
| [`provider.detect`](#providerdetect) | 查找现成的配置：环境变量里设置了的密钥、本机正在运行的模型服务 |
| [`provider.catalog`](#providercatalog) | 搜索供应商目录 |
| [`provider.test`](#providertest) | 测试一家供应商能不能连通 |
| [`provider.rename`](#providerrename) | 修改一家供应商的编号 |
| [`model.call`](#modelcall) | 调用一次模型，不进入任何会话 |
| [`model.embed`](#modelembed) | 计算一批文字的向量 |
| [`usage.query`](#usagequery) | 查询用量和金额 |
| [`usage.convert`](#usageconvert) | 把你自己记录的金额换算成 `usage.currency` |

## 配置的三层

配置从下往上叠加：出厂默认值 → 系统配置（`config.toml`，你的设置都写在这里）→ 项目配置（仓库里的 `.miyu/config.toml`，只有信任过才生效，并且只能收紧设置）。每一项配置能写在哪几层，写在配置清单的 `layers` 里。密钥另外存放在 `secrets.toml` 里，配置里写 `{ secret = "<名字>" }` 来引用它，也可以写 `{ env = "<环境变量>" }` 引用一个环境变量。

在这三层之上，还有这一次启动核心时的环境变量，并且只有两项配置接受环境变量：`log.level` 读取 `MIYU_LOG`，`models.catalog.update` 读取 `MIYU_CATALOG_UPDATE`。如果设置了环境变量，并且它的值能被识别，就使用环境变量的值，此时 `origin` 的 `layer` 是 `env`；选项不区分大小写，开关只接受 `true`、`false`（不区分大小写）；无法识别的值不生效，仍然使用配置里的值。

**问题**：读取、检查、修改配置时发现的每一处问题写成一条，包含这些字段：`level`（`error` 或 `warning`）、`code`（原因码）、`file`、`line`、`column`、`key`、`expected`、`got`、`suggest`（键名拼错时，和它最接近的那个键）、`using`（现在实际使用的是什么）、`message`（给人看的一整句话，使用连接的语言）。不适用的字段不写。`got` 原样抄录收到的内容，最多 80 个字符；但是对于类型为密钥的项（以及列表里的密钥字段）、不认识的键和字段、把一组键写成了一个值的情况，`got` 不抄录原文，`got` 和 `message` 里只描述写的是什么样的内容，例如「24 个字符的字」，原值不会出现在回应里。

**谁能调**：读取类的方法（`config.schema`、`config.get`、`config.check`、`secret.list`、`model.list`、`provider.*`、`model.call`、`model.embed`、`usage.query`、`usage.convert`）人和扩展都能调用。修改类的方法（`config.trust`、`secret.set`、`secret.delete`）只给人调用，扩展调用时返回 `local_only`。对于 `config.set`，扩展只能逐项修改系统配置里由它自己的清单声明的项，并且这些项不能是密钥，也不能写成引用。

## `config.schema`

**参数**：`keys`（字符串数组）：只返回这几项；不填写时返回全部。

**回应**：`items`（每一项配置，按清单里的先后排列）、`pages`、`groups`（设置页的页和组）。每一项包含：

| 名字 | 说明 |
|---|---|
| `key`、`type` | 键和类型。类型有 `bool`、`option`、`int`、`float`、`text`、`duration`、`path`、`url`、`name`、`reference`、`model`、`model_or`、`english`、`secret`、`list`、`table`；软件包的配置项还有 `persona`、`knowledge`、`preset`（分别用来选人格、选知识库、选预设；选模型的项是 `reference`）。键里由人起名字的那一段写成占位符 `<id>`、`<model>`、`<external>`、`<name>`，例如 `providers.<id>.base_url` |
| `options` | 选项：每个选项有 `value` 和给人看的 `name`；写了说明的选项带 `description`（用暗色显示在下面）；带 `notice` 的选项被选中时会弹出这一句提示，提示只有一个确认按钮，选项仍然会保存；核心能检查出来的情况下，选项带 `note`（用暗色显示在名字后面）、`available: false`（表示不能使用，显示成灰色） |
| `min`、`max`、`max_chars`、`element` | 取值范围、长度、列表元素的类型（`record` 表示每一项是一组字段） |
| `title`、`fields` | 元素是 `record` 时：`title` 是拿来当每一项标题的字段（没有时显示成「第几项」）；`fields` 里每个字段一项，格式和这张表相同（`key` 是字段名） |
| `default` | 默认值 |
| `layers` | 能写在哪几层：`system`、`project` |
| `applies` | 什么时候生效：`now` 是立即生效，`next_turn` 是下一轮生效，`new_session` 是以后新开的会话生效，`restart` 是重启核心以后生效，`head_start` 是头（连接核心的界面程序，例如终端界面、网页界面）下次启动时生效 |
| `name`、`description` | 给人看的名字和说明，使用连接的语言 |
| `name_en` | 英文的名字：只有连接的语言不是英文时才有，头用它和 `name` 一起搜索（在中文界面里搜索 web 能找到「网络搜索」）；没有英文名字的项不带这一格 |
| `page`、`group`、`common`、`control` | 在设置页里放在哪里、是不是常用项、用什么控件。控件有 `select`、`toggle`、`number`、`text`、`list`、`combo`、`textarea`（多行的文本框）、`slider`（滑块） |
| `step` | 控件是 `slider` 时：每一格的大小（整数类型的项，这个值是整数） |
| `suggestions` | 控件是 `combo` 时：建议的值 |
| `hidden` | 设置页里不显示这一项 |
| `package` | 软件包的配置项：属于哪个软件包 |
| `section` | 软件包分了组的配置项：它在这个包的哪一节（值是组名，见下面 `groups` 里的 `sections`）；不在组里的配置项不写这一格 |
| `choices_from` | 软件包的文字项，选项由软件包在运行时提供：`{"package", "method"}` 指明是哪个包的哪个方法；清单没有写的项不带这一格，详细的见下面 |

每一组包含 `id`、`name`（使用连接的语言）、`name_en`（英文的名字，只有连接的语言不是英文时才有）、`page`；写了说明的组多一格 `description`（一句或几句话，使用连接的语言，显示在组名下面），没有写说明的组不带这一格。如果软件包把配置项分了组（清单里写了 `[settings.<组>.<名字>]`），这个包的那一组多一格 `sections`：每一节是一个 `{"id", "name", "shown_when"?}`，按清单里写的先后排列。`name` 使用连接的语言；清单没有写名字时，`name` 是组名。`shown_when` 是 `{"key": "<完整的键>", "is": ["<值>", …]}`：只有那一项现在的值是 `is` 里的某一个时，才显示这一节，`is` 总是列表；没有 `shown_when` 的节总是显示。那一项可以是选项，也可以是带 `choices_from` 的文字项（`is` 里的值不一定在它的选项里），两种情况都只比较值。不显示的节只是不显示，节里的项仍然能写、能读，也照常检查。没有分组的软件包没有 `sections`。

带 `choices_from` 的项：类型仍然是 `text`，头把它显示成下拉框，选项通过调用 `package.call` 获取（参数是 `{"package", "method"}`，按这一格原样填写，不带 `params`），软件包返回 `{"choices": [{"value", "name"?, "description"?}]}`：`value` 是存进配置的文字；没有写 `name` 时显示 `value`；`description` 用暗色显示在下面。如果调用失败（`program_not_running`、`unregistered`、`method_failed`、`method_timeout`），或者返回的 `choices` 是空的、格式不对，头就退回成文字框；存着的值不在列表里时也照常显示。核心不按列表检查值，写的是什么就存什么。

**包的「启用」**：每个软件包（和 `package.list` 里的 `enabled` 相同，每个包都有开关）各有一项 `packages.<编号>.enabled`：`type` 是 `bool`，`control` 是 `toggle`，`applies` 是 `now`，`page` 是 `packages`，`group` 和 `package` 是这个软件包，排在这个包自己的配置项前面。`true` 表示开启。它就是 `package.enable`、`package.disable` 操作的那个开关，不写在配置文件里。以前的 `packages.<编号>.off`（只对某个人关闭）已经删除，任何地方都不再接受。

**出错**：`unknown_config_key`：`keys` 里有清单里没有的键时返回，`data.problems` 里每个这样的键一条，并带上和它最接近的键名 `suggest`。

## `config.get`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `keys` | 字符串数组 | 只返回这几项；不填写时返回全部 |
| `cwd` | 字符串 | 从这个目录查找项目配置，并算进最终值；不填写时不算项目配置 |
| `all` | 布尔，默认 `false` | 是否为每一项再列出写了它的每一层 |

**回应**

| 名字 | 说明 |
|---|---|
| `items` | 键 → `{value, origin}`：最终值和来源（`origin` 包含 `layer`、`file`、`line`）。填写了 `all` 时多一格 `layers`：从上往下每一层一个 `{origin, value, used}`，没有生效的层带 `problem` |
| `files` | 每一层的文件：`system`、`project`（包含 `file`、`version`；项目配置还多一格 `trusted`，值是 `true`、`false`，或者 `null` 表示还没有问过），`secrets`（只有 `file`） |
| `problems` | 这几份文件现在的全部问题 |

```json
{"files":{"secrets":{"file":"secrets.toml"},"system":{"file":"config.toml","version":"sha256:…"}},"items":{"ui.language":{"origin":{"file":"config.toml","layer":"system","line":3},"value":"zh"}},"problems":[]}
```

包的「启用」返回 `{"origin": {"layer": "package"}, "value": true}`，来源是 `package`，表示它不在任何一层配置里；填写了 `all` 时，`layers` 是 `[]`。

软件包带来的供应商（清单里的 `[providers.<编号>]`）的各项配置返回 `{"origin": {"layer": "package", "package": "<包>"}, "value": …}`，这一层在默认值之上、配置文件之下，不会写进配置文件；软件包卸载或者关闭以后，这些项就没有了。如果人修改其中一项，修改会写进配置文件，盖住软件包给的值（核心会顺便写上 `providers.<编号>.package = "<包>"`）；如果配置文件里原来就有同一个编号、没有写 `package` 的供应商，它是人自己配置的供应商，此时软件包带来的那一家整个不出现。软件包这一层变化时推送 `config.changed`，其中 `via` 是 `package`。

借来的值返回 `{"origin": {"layer": "inherited", "from": "providers.deepseek.key"}, "value": null}`：表示哪一层都没有写这一项，使用的是 `from` 那一项的值（界面上显示成「已继承」）。现在只有 `net.search.deepseek.key` 是这样：没有填写时，它借用模型那边官方 DeepSeek 供应商（地址没有写，或者是 `api.deepseek.com`）的密钥，`from` 是那一家的 `providers.<编号>.key`；中转站的密钥不借用，此时来源仍然是 `default`。返回的值里不包含密钥。`config.set` 的回应、`config.changed` 推送里也同样这样写；修改供应商的密钥时，不会另外推送这一项，需要显示它的头在收到含有 `providers.` 的 `config.changed` 时再读取一次。

**出错**：`unknown_config_key`。

## `config.set`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `layer` | `system`，可以不写 | 修改哪一层：只能是系统配置（填写其他值返回 `bad_params`） |
| `changes` | 数组 | 要修改的几项，每一项的写法见下面 |
| `text` | 字符串 | 把整份文件替换成这段文字 |
| `version` | 字符串或 `null` | 填写了 `text` 时必须填写：你读到的这份文件的版本；`null` 表示读的时候还没有这份文件 |

`changes` 和 `text` 必须正好填写一个。`changes` 的每一项包含：`key`（必须填写；元素是 `record` 的列表只能整条一起替换，不能写成 `<键>[0].<字段>`）；`value`（JSON 的值；`record` 的列表写成对象的数组，其中的密钥字段只接受 `{"secret": …}`、`{"env": …}`，写了明文的整条拒绝，问题的 `key` 是 `<键>[第几项].<字段>`）、`input`（人输入的文字，按这一项的类型解析）、`unset: true`（恢复默认值），这三个必须选一个；`expect`（可以不写）：这一层这一项现在应当是 `{"value": …}`，或者 `{}`（没有写），对不上时拒绝。

**回应**：`{"keys", "version"}`。`keys` 里是变化了的每一项，包含 `value`（这一层现在的值，删掉的项不写）、`effective`、`origin`（算上其他层以后的最终值和来源）、`applies`。如果一项都没有变化，`keys` 是空的，也不会写入任何内容。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"config.set","params":{"changes":[{"key":"ui.language","value":"zh"}]}}
← {"id":"s1","jsonrpc":"2.0","result":{"keys":{"ui.language":{"applies":"now","effective":"zh","origin":{"file":"config.toml","layer":"system","line":3},"value":"zh"}},"version":"sha256:…"}}
```

几项一起检查、一起写入：只要有一项不对，整条请求都不接受。写入磁盘、推送 `config.changed` 以后才返回回应。

修改包的「启用」（`packages.<编号>.enabled`）就是开关这个软件包，和 `package.enable`（不带 `approve`）、`package.disable` 相同：不写配置文件，也不推送 `config.changed`，`keys` 里那一项是 `{"applies": "now", "effective": <这时开没开>, "origin": {"layer": "package"}, "value": <同 effective>}`；由于来源是 `package`，`miyu config set` 不会提示「被上面一层压着」。这一项只接受 `value` 填写 `true`、`false`，或者使用 `input`；`unset` 返回 `bad_params`，`expect` 会被忽略。如果和其他项写在同一条请求里，核心会先把其他项写进配置，再开关软件包；如果软件包无法开启，返回 `package.enable` 的原因（`needs_approval`、`program_missing`……），已经写进配置的项会保留。扩展修改这一项时返回 `local_only`。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `changes`、`text` 没有正好填写一个；某一项里 `value`、`input`、`unset` 没有正好填写一个；同一个键写了两次；填写了 `text` 但是没有填写 `version` |
| `unknown_config_key` | 清单里没有这个键，详细的在 `data.problems` 里 |
| `config_invalid` | 值不对，或者这一项不能写在这一层；`data.problems` 列出每一处 |
| `config_conflict` | `expect` 对不上（见 `data.current`）；`version` 对不上，或者一直有人在手动修改这份文件（见 `data.version`） |
| `config_file_broken` | 这份文件现在无法读取，所以不能只修改其中几项；详细的在 `data.problems` 里 |
| `local_only` | 扩展修改了不允许它修改的项 |

## `config.check`

**参数**：`layer`（`system` 或 `project`，必须填写）、`text`（必须填写）。**回应**：`{"problems": [...]}`，没有问题时是空数组；这里的问题不带 `file`。

## `config.trust`

**参数**：`cwd`（必须填写，核心从这个目录查找项目配置）、`version`（必须填写，是人看过的那一份配置的版本，即 `config.get` 返回的 `files.project.version`）、`trust`（布尔，必须填写）。

**回应**：`{"file", "trusted"}`。只能信任人看过的那一份配置。

**出错**：`no_project_config`；`config_conflict`（人看过以后文件又变了，`data.version` 是现在的版本）。

## `secret.set`、`secret.delete`、`secret.list`

| 方法 | 参数 | 回应 |
|---|---|---|
| `secret.set` | `name`、`value`（都必须填写） | `{"replaced": <原来有没有>}` |
| `secret.delete` | `name` | `{}` |
| `secret.list` | 没有 | `{"secrets": [{"name", "set", "used_by"}…]}` |

存入的值会去掉前后的空白。`secret.list` 从不返回密钥的值：它列出已经设置了的密钥，以及配置里引用了、但是还没有设置的密钥；`used_by` 是引用这个密钥的配置键。

密钥只存放在一个文件里：`secrets.toml`。

**出错**：`bad_params`（名字不符合写法要求；值是空的、含有控制字符，或者超过 16 KiB）；`unknown_secret`（要删除的密钥不存在）；`config_file_broken`（密钥文件无法读取）；`local_only`（扩展调用了 `secret.set`、`secret.delete` 这两个修改类的方法）。

## `model.list`

**参数**：`provider`（只查看这一家供应商）、`refresh`（布尔，先拉取一遍供应商的模型列表再回答）、`models`（字符串数组，只返回这几个 `<供应商>/<模型>`）。

没有填写 `refresh` 时，核心按手头已有的列表回答，并在后台拉取：没有填写 `models` 时，每家供应商最多一分钟拉取一次；填写了 `models` 时，只拉取还没有列表、或者列表已经超过一天的供应商。如果拉取到的列表有变化，订阅了 `config` 流的连接会收到 [`models.changed`](pushes.md)。

**回应**

| 名字 | 说明 |
|---|---|
| `providers` | 配置好的供应商，按编号排序。每家供应商包含：`id`、`name`（`{value, from, key}`）、`driver`、`base_url`、`key`（`{ref, set, state}`，从不返回密钥的值）、`catalog`（对应目录里的哪一家）、`logo`（`{svg, tint}` 或 `null`）、`models`；无法使用的供应商另外带 `problem`。`needs` 是这家供应商真正需要的两项：`{"key", "base_url"}`，每项是 `none`（不需要，界面不显示这一项）、`optional`（可以不填）、`required`（必须填）；使用扩展驱动的供应商照驱动在清单里的声明，扩展没有安装、驱动名对不上时没有这一格。软件包自带的供应商另外带 `package`（包的编号）：界面把包给的值显示成默认值，人仍然可以覆盖 |
| `models` 里每个 | `model`、`ref`、`listed`（从哪几处列出：`config`、`provider`、`catalog`）、`facts`（窗口、最大输出、输入种类、能不能使用工具、思考、价格、名字……每一格都有 `value` 和来源）、`state`（`ok`、`cooling`（这时带 `until`/`class`）、`no_key`）；能输出向量的模型带 `embedding: true` |
| `pools` | 每个模型池包含：`name`、`strategy`、`models`、`usable`、`subagent`、`description`、`fixed`（`lite`、`normal`、`flagship` 这三个固定池的 `fixed` 是 `true`：它们永远存在，配置里没有写也会列出来，不能删除、不能改名，`subagent` 默认是 `true`） |
| `side` | 旁路请求（不在对话里的那些模型请求）的登记，每条包含：`id`（`core/title` 表示起标题，`memory/organize` 表示整理记忆）、`tier`（需要哪一档：`lite`、`normal`、`flagship`）、`name`、`summary`（使用连接的语言）、`resolved`（这一刻请求会落到哪里：这一档的池有成员时是池名，池是空的时是 `session`，表示使用会话的模型）、`package`（由扩展登记的旁路请求写明是哪个软件包登记的，出厂的没有这一格）。如果池里的模型全都在冷却或者都出错了，请求也会退回使用会话的模型。扩展在清单的 `[[side]]` 里登记的旁路请求也列在这里，`id` 是 `<包>/<编号>`，名字和说明使用它的清单里写的内容 |
| `uses` | `chat`、`vision` 各自配置的引用，没有配置的是 `null` |
| `catalog` | 正在使用的目录：`source`、`fetched`；目录无法读取时是 `null` |

**出错**：`unknown_provider`；`bad_params`（`models` 的写法不对）。

## `provider.detect`

**参数**：没有。**回应**：

| 名字 | 说明 |
|---|---|
| `keys` | 核心的环境变量里设置了的密钥：`env`、`provider`、`name`、`driver`、`supported`，已经配置过的带 `configured`。不返回密钥的值 |
| `local` | 本机正在运行的模型服务：`provider`、`name`、`base_url`、`models`，已经配置过的带 `configured` |
| `looked_for` | 查找了哪些环境变量的名字 |

## `provider.catalog`

**参数**：`query`（编号或名字里包含这一段文字的供应商）、`limit`（默认 50）、`featured`（布尔，只返回常用的几家，按连接的语言挑选国内或国外的供应商）。

**回应**：`{"providers": [...]}`，每家供应商包含 `id`、`name`、`driver`、`base_url`、`env`、`doc`、`models`（有几个模型）、`supported`、`local`（地址在本机，不需要密钥）、`logo`。能使用的供应商排在前面。

## `provider.test`

**参数**：`provider`（已经配置好的供应商）或 `candidate`（还没有写进配置的供应商，包含 `driver`、`base_url`、`key`（`{secret}`、`{env}` 或 `{value}`，`value` 只在这一次使用，不会保存）、`catalog`），两者必须正好填写一个；`model`（用哪个模型来测试）。

能列出模型就算连通，不发送请求；只有列不出模型（并且原因不是密钥错误）、或者填写的模型不在列表里时，才发送一句话，这时会花费一点额度。如果列不出模型，又没有填写模型，核心从目录里这一家的模型中挑一个推荐的模型来发送；只有目录里也没有这一家时，才返回 `need_model`。

**回应**：成功时返回 `{"ok": true, "checked", "models", "listed", "model"}`，发送过一句话时多一格 `first_token_ms`；失败时返回 `{"ok": false, "stage", "error": {"class", "status"?, "message"}}`，`stage` 是 `list`、`request`、`config` 之一；如果给出一个模型名就能再试，带 `need_model: true`。

```json
{"ok":true,"checked":"list","models":["deepseek-flash","deepseek-v4-pro"],"listed":"provider","model":"deepseek-flash"}
{"ok":false,"stage":"request","error":{"class":"auth","status":401,"message":"Authentication Fails (no such user)"}}
```

**出错**：`bad_params`；`unknown_provider`。

## `provider.rename`

这个方法一次改完一家供应商的编号：配置里的 `[providers.<from>]` 那张表（连同子表、密钥的引用）换成 `<to>`，`models.chat`、`models.vision`、`models.embedding`、池的成员里的 `<from>/<模型>` 换成 `<to>/<模型>`；使用旧编号的会话从下一轮起使用新编号。只给人调用。

**参数**：`from`、`to`。`to` 要符合编号的写法：以小写字母开头，由小写字母、数字、`-`、`_` 组成，最多 32 个字符。

**回应**：`{}`；修改了的每一层都会推送 [`config.changed`](pushes.md)。

**出错**：`bad_params`（`to` 的写法不对，或者和 `from` 相同）；`unknown_provider`（不存在 `from`）；`provider_exists`（已经存在 `to`）。如果两层里后一层写入失败，前一层已经修改的内容不会撤回，返回后一层的错误（`config_conflict`、`config_file_broken`），再调用一次会把剩下的部分改完。

## `model.call`

调用一次模型，拿到完整的回答和用量。这个调用不进入任何会话，也不推送；但是会花费额度，并且记进用量。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `purpose` | 字符串，必写 | 用途，由 1 到 32 个小写字母、数字、`-` 组成，例如 `vision` |
| `messages` | 数组，必写 | `{"role": "system"|"user"|"assistant", "text", "images"?}`。`system` 最多一条，并且只能放在最前面；最后一条是 `user`；只有 `user` 能带 `images`，它是 blob 的哈希（用 `blob.put` 存入的），不存在的 blob 返回 `unknown_attachment` |
| `model` | 字符串 | `<供应商>/<模型>` 或 `@<池>`；不填写时使用 `models.chat`。三个固定的池（`@lite`、`@normal`、`@flagship`）是空的时，按没有填写处理，同样使用 `models.chat` |
| `use` | `chat` 或 `vision` | 不知道人配置了哪个模型时使用：分别对应 `models.chat` 或 `models.vision`；和 `model` 最多填写一个 |
| `side` | 字符串 | 扩展的旁路请求：填写自己软件包的清单里 `[[side]]` 登记的编号（不带包名）。核心按登记的档位从同名的池（`lite`、`normal`、`flagship`）里取模型；池是空的、池里的模型全都在冷却或者都出错了时，使用会话的模型；会话的模型也是一个空的固定池时，使用 `models.chat`。不能和 `model`、`use` 同时填写；人的连接填写这一项，或者填写的编号没有登记过，返回 `bad_params` |
| `session` | 会话编号 | 只和 `side` 一起使用：用不上档位池时，使用这个会话这一刻的模型；不填写时使用 `models.chat`。填写的不是自己的会话时返回 `session_not_found` |
| `max_tokens` | 正整数 | 最多输出多少 token |

**回应**：`{"text", "provider", "model", "usage"}`。`usage` 包含 `uncached`、`cache_read`、`cache_write`、`output`，供应商报告了思考用量时多一格 `reasoning`。供应商没有报告的项是 `null`。这个方法在后台回答，不会挡住这个连接上的其他请求。

```json
{"text":"A cat on a red sofa.","provider":"deepseek","model":"deepseek-flash","usage":{"uncached":812,"cache_read":0,"cache_write":0,"output":9}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 参数的写法不对；图片的 blob 不是图片；`side` 不是这个扩展登记过的编号 |
| `unknown_attachment` | 不存在这个图片的 blob |
| `unknown_model` | 无法解析 `model`：写法不对、没有这家供应商、没有这个池，或者自己建的池是空的 |
| `no_model` | 没有能使用的模型（没有配置，或者取不到密钥），说明在 `data.message` 里 |
| `cooling` | 候选的模型全都在冷却，见 `data.message`、`data.wait_ms` |
| `model_failed` | 请求已经发送，但是出错了：见 `data.class`、`data.status`、`data.message` |

## `model.embed`

**参数**：`texts`：1 到 64 条文字，每条 1 到 8000 个字。核心按 `models.embedding` 计算（本机的内置模型，或者远程的一家供应商）。

**回应**：`{"model", "vectors"}`。`model` 是模型编号（`local:<id>` 或 `<供应商>/<模型>`）：换了模型以后，新的向量不能和以前的向量比较。`vectors` 按 `texts` 的先后排列，并且已经归一化。

**出错**：`bad_params`；`embedding_off`（向量功能关闭了，或者要求使用本机的模型，但是没有安装内置模型）；`embedding_failed`（有一条算不出来，或者等待超时，原因在 `data.why` 里）。

## `usage.query`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `from`、`until` | 时刻 | 只统计这一段时间，包含开始时刻，不包含结束时刻 |
| `group` | 数组 | 按这几样分组：`venue`、`model`、`day`、`session`、`purpose`、`use`（用途：`main` 主对话、`compaction` 压缩、`subagent` 子代理、`recap`、`title`、`memory`、`other`）；不填写时返回一行总计 |
| `session` | 字符串 | 只统计这个会话；填写 `tree: true` 时连同它派出的子会话一起统计 |
| `offset` | 时区，例如 `+09:00` | 分天时使用哪个时区；不填写时使用核心所在机器的时区 |

**回应**：`{"rows": [...]}`。每一行包含：分组用的那几格（按 `purpose` 分组时另外带 `purpose_name`，是这个用途给人看的名字，使用连接的语言：核心自己的用途使用核心的说法，`purpose` 是 `null` 的那一行是主对话，扩展写的用途使用它的清单里 `[[purpose]]` 声明的名字；都没有名字时不带 `purpose_name`，界面按 `purpose` 原样显示）、`requests`（请求数）、`usage`（四项用量加起来的结果）、`amounts`（按币种各自累加，保持原样）、`unpriced`（有用量但是算不出金额的次数）、`converted`（换算成 `usage.currency` 以后的一个数：每一笔按它那一天的汇率换算，格式是 `{"currency","amount"}`；取不到汇率的那几笔放在它的 `rest` 里，写法和 `amounts` 相同；一笔都没有换算成功时不写这一格）、`converted_parts`（换算成功的那几笔按四种用量分别是多少：`{"uncached","cache_read","cache_write","output"}`，币种和 `converted` 相同，四种都在时加起来就是 `converted.amount`；用量是 0 的那一种是准确的 0；如果有一笔用到了某一种、而那一种的价格又不知道，那一种不写；如果这一行里有旧的、没有分成四种记录的账，整个 `converted_parts` 不写；没有 `converted` 时也不写）。每一种都按那一次请求自己记下的价格算好，跟着那一次请求记账，以后修改价格也不会变。所有会话都会记账。金额按配置的 `usage.currency` 排序、换算。如果用到了 open.er-api.com 的汇率，回应的顶层多一格 `rates`（来源和链接），界面要注明汇率的来源。

```json
{"rows":[{"purpose":"vision","purpose_name":"多模态","model":"deepseek/deepseek-flash","requests":2,"usage":{"uncached":1624,"cache_read":0,"cache_write":0,"output":18},"amounts":[{"currency":"USD","amount":0.0002544}],"unpriced":0,"converted":{"currency":"CNY","amount":0.00181896},"converted_parts":{"uncached":0.00174174,"cache_read":0,"cache_write":0,"output":0.00007722}}]}
```

这一行里缓存写入的价格不知道，但是它的用量是 0，所以 `cache_write` 仍然是 0；四种加起来就是 `converted.amount`。

**出错**：`bad_params`。

## `usage.convert`

这个方法把你自己按日期、币种记下的金额交给核心，换算成 `usage.currency`。扩展自己记账（例如按人记账）时使用它：汇率由核心获取、缓存，换算的方法和 `usage.query` 的 `converted` 相同。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `groups` | 数组的数组 | 一组是要合成一个数的那几笔，可以是空的。最多 1000 组，所有组合计最多 20000 笔 |

每一笔包含：

| 名字 | 类型 | 说明 |
|---|---|---|
| `day` | 字符串 | 这一笔是哪个 UTC 日期，写成 `2026-11-10` 这样；按这一天的汇率换算 |
| `currency` | 字符串 | 币种代码，由三个字母组成，例如 `USD` |
| `amount` | 数 | 金额 |

**回应**：`{"currency", "converted"}`。`currency` 是这一刻的 `usage.currency`；`converted` 按 `groups` 的先后每组一项，写法和 `usage.query` 一行里的 `converted` 相同（`{"currency","amount"}`，取不到汇率的那几笔放在 `rest` 里，用到了 open.er-api.com 的汇率时带 `rates`，界面要注明来源）；一笔都没有换算成功的组（包括空的组）是 `null`。缺少的汇率会当场获取，最多等待两秒。

```json
{"groups":[[{"day":"2026-11-09","currency":"USD","amount":0.12},{"day":"2026-11-10","currency":"CNY","amount":0.5}],[]]}
```

```json
{"currency":"CNY","converted":[{"currency":"CNY","amount":1.3544},null]}
```

**出错**：`bad_params`（日期、币种的写法不对，金额不是数，或者超过了上限）。

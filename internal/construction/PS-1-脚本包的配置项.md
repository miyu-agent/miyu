# PS-1：脚本包的配置项

2026-10-30 项目主人派给核心，要的是 miyu-agent-41（生图脚本包 `miyu-scripts-ai-draw`：接口地址、API key 这些要人填）。形状 core1 和 41 定好：只带脚本的包（`bundle`）也能写 `[settings]`，跑这个包的脚本时把它自己的配置值交给脚本。

## 目的

- `bundle` 能写 `[settings.<名字>]`，规矩、画法同 `process` 包那一张表：类型、`default`、`layers`、`applies`、`name`、`description`、`hidden`，`secret` 不能写默认值；画在「软件包」页这个包那一组。
- 包自己 `scripts/` 下的脚本另拿到 `MIYU_SETTINGS_JSON`：这个包配置项这一刻的最终值，一个 JSON 对象，键去掉包的配置前缀（`[package] prefix`，没写的是包编号；有组的是 `组.名字`），密钥是真值，没设的键不放。每次调用现读：改了的下一次调用就换。
- 别的包的脚本拿它自己那份；家目录 `scripts/` 里人自己放的、她跑的 `shell` 都没有。值不进运行日志、不进时间线；沙盒里照旧读不到配置、密钥文件。

## 蓝图改哪几节

- `packages.md`：种类那张表 `bundle` 一行多 `[settings]`；「配置项」开头写明 `bundle` 也能写，第 5 条脚本包。
- `skills.md`「脚本」第 3 条：`MIYU_SETTINGS_JSON` 给谁、是什么、什么时候取。
- 作者文档 `docs/authors/scripts-skills.md` 中英：「怎么跑」里一段脚本包的配置项，带例子。

## 怎么做

1. `miyu-config` 的 `kinds.rs`：`Bundle` 收 `settings`。登记配置项（`packages::settle`）本来不分种类，照清单生成、挂在「软件包」页这个包那一组。
2. 值：同扩展握手交的那一份（`extensions/config.rs` 的 `own`，密钥照配置服务手里的密钥文件解，不另读盘），键去掉第一段。`miyu_session::ConfigSource::package_settings`（端点的 `Config` 实现）、`miyu_tool::SettingsPort::package`（会话的 `Turn` 实现）各加一个带默认实现的方法，`Call` 不加格。
3. 现读：会话的一轮（`Turn`）另留配置服务的 `watch`，`package` 照这一刻的那一份取，不照这一轮冻结的；自己造的一轮（一次性调用）照造它的那一份。
4. 脚本：`script.rs` 来处不是 `~home` 的，照端口取这个包的那一份，写成 JSON 放进 `MIYU_SETTINGS_JSON`；没有的不放。运行日志只记 `DEBUG script settings given`。

## 不做什么

- 不带项目配置：包的配置项只能写在系统配置（`layers` 只有 `system`）。
- 别的包、家目录里的脚本、`shell` 一律不给，不做「借用别人的配置」。
- 不加专门的协议方法：脚本照环境变量读。

## 验收

1. `miyu-config`：`a_bundle_can_declare_settings`（bundle 写配置项读得成、密钥写默认值不收），原来「不带程序的包没有配置项」那条断言去掉。
2. 端点 `script_settings`：bundle 的配置项登记在「软件包」页这个包那一组；交出的键去掉前缀、有组的写 `组.名字`、密钥是真值、没设的不放；没有配置项的、没装的包没有；写了 `prefix` 的照前缀去掉。
3. 会话 `config::tests`：同一轮里配置服务换了新的一份，下一次取就是新的；自己造的一轮照造它的那一份。
4. 基础系统 `script_settings`：包自己的脚本拿到它那一份，别的包的拿它自己那份，家目录的、没有配置项的包没有，改了的下一次调用就换；`shell` 没有（Unix）。
5. X-47 的「文档和代码的环境变量对得上」多 `MIYU_SETTINGS_JSON`。
6. 手写变异、本地门禁、三平台 CI。

## 风险

- 环境变量的长短：Windows 上一个环境变量最多 32767 个字。配置项是人填的几项，远够；真超了起不来，同 `MIYU_ARGS_JSON` 那样会报起不来。
- 密钥给了脚本：只给这个包自己的脚本，同扩展握手的例外（`config.md` 第九条「密钥不经协议交出去」）；脚本是装包时人批过的代码。

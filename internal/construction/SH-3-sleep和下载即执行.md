## 施工单 SH-3：只拒开头就睡的 `sleep`，下载即执行不再误报

状态：做完，等验收（分支 `step/SH-3-sleep和下载即执行`）。两件都是 Terminal-Bench 实测撞上的（2026-11-27 core1 派）。

### 目的

1. 她等服务起来写的 `postfix start; sleep 2; postfix status`、`server & sleep 3 && curl …` 照跑；开头就睡的照旧不跑，拒的时候说清等服务的办法。
2. `TOKEN=$(curl …); python3 check.py "$TOKEN"` 这种拿下载的字当数据用的，不再判成下载即执行、要人确认（无人值守时就是拒）。

### 经过

1. 2026-11-27 项目主人先定只拒整条就是一个 `sleep` 的，同日改定：照顶层切段以后，**第一段**是不短于 2 秒的前台 `sleep` 就拒（整条只有它的、`sleep 5; curl …`、`sleep 10 && check`、`sleep 3` 换行再接命令的都算），`sleep` 不在第一段的照跑；管道、循环体、后台里的照旧照跑。拒绝的文案去掉「replies from other sessions」，补一句等服务就绪的办法。
2. 2 秒的门槛、切段的认法（`;`、`&&`、`||`、换行、单个 `&`；引号、括号里的不算）是主会话定的技术细节，照 10-26 那一版不动。
3. 下载即执行是纯技术 bug：`hazard/rules.rs` 对每段替换判「里面在下载」以后，`fed::pipeline_feeds_shell` 看的是整条命令里有没有一个简单命令是解释器，不是装着这段替换的那一条。core1 给的真实样本（Terminal-Bench `git-multibranch`，一条多行命令：`python3 - <<'PY' … PY` 跑自己的脚本，后面另一行 `echo "main: $(curl -s -o /dev/null -w '%{http_code}' https://…) -> $(curl -s https://…)"`）用 main 上的 `miyu_policy::hazard::assess` 跑出来是 `Asks([PipeToShell])`，只撞了这一条规则；无人值守的会话里成了 `core/tool-results/unattended`。

### 照的图纸

- `internal/blueprint/tools/shell.md`「怎么走」第 4 条、「高危命令」第 5 条，守着它的表；`internal/blueprint/session/guard.md` 守着它的表；设计 26 登记簿 `no-sleep.txt` 一行；`docs/{en,zh}/guide/tools.md`「干等」。

### 改了什么

- `crates/miyu-basesystem/src/shell/waiting.rs`：`waits` 只看切出来的第一段（命令先去掉前后空白）：不在后台、就是 `sleep <时长…>`（`Start-Sleep`）、不短于 2 秒。第一段接着管道的（`sleep 3 | cat`）不算。
- `resources/software/basesystem/shell/no-sleep.txt`：

  ```text
  Not run: no need to wait. Finished background jobs reach you after this turn ends. A loop with a check can wait for a service to come up.
  ```

  指纹 `deda3c81` → `3a6fa933`，o200k 23 → 32。照 J13 写成事实，不写命令口吻；不说是哪一段（拒的总是第一段）。人看的 `shell/no-sleep` 三语照实改：「不允许开头就用 sleep 等待」「starting with a sleep isn't allowed」「sleep から始めて待つことはできません」。`cargo xtask prompts` 重生成了 `internal/blueprint/prompts.md`。
- `crates/miyu-policy/src/hazard/lex.rs`：`Simple` 多记 `substitutions`，这一条里每段替换的原文：词里的、`< <(…)` 读的、here-string 里的都算。
- `crates/miyu-policy/src/hazard/fed.rs`：`pipeline_feeds_shell` 换成 `runs_fetched(simple)`：这一条有一段替换在下载，它自己（剥掉外壳）是 shell、解释器、`source`、`.`、`eval`。`fetch_into_shell` 把「前面那条的替换里在下载」也当下载（`cat <(curl …) | bash`、`echo "$(curl …)" | sh`）。
- `crates/miyu-policy/src/hazard/rules.rs`：每个简单命令照 `runs_fetched` 判，替换那一圈只另认里面的。

### 不做什么

- 解释器那一条里替换的位置不细分：`python3 a.py "$(curl …)"` 照旧问（下载的字当参数，不一定是脚本），要细分得认每种解释器的选项，误判的代价是多问一次。
- 不认 heredoc 正文里的替换（`python3 - <<EOF` 里写 `$(curl …)`）：拆命令时正文不拆，修以前也认不出。
- PowerShell 那一路（`hazard/windows.rs`）自己有一份 `fetch_into_shell`，没有这个误报，不动。

### 验收

1. `crates/miyu-basesystem/src/shell/waiting/tests.rs`：拒的加上 `sleep 5; curl …`、`sleep 10 && check`、`sleep 3` 换行 `ls`、`sleep 30;`、`sleep 5 > /dev/null`、开头带换行的、`Start-Sleep 5; Get-Date`；照跑的加上 `postfix start; sleep 2; postfix status`、`server & sleep 30 && curl …`、`sleep 5 | cat`、`echo start; sleep 10`、`make || sleep 5`、`cd /tmp` 换行 `sleep 3`、`sleep 30 & sleep 30`。
2. `crates/miyu-policy/src/hazard/tests/fed.rs`：`a_download_run_by_an_interpreter_is_reported` 11 条照报（`bash <(curl …)`、`python3 <(curl …)`、`sh -c "$(curl …)"`、`sudo bash -c "$(wget …)"`、`curl … | python3 -`、`source <(curl …)`、`bash < <(curl …)`、`bash <<< "$(curl …)"`、`cat <(curl …) | bash`、`echo "$(curl …)" | sh`、`cd … && bash <(curl …)`）；`a_download_used_as_data_is_left_alone` 5 条不报，最后一条是 Terminal-Bench 那条缩成的（主机名换 `example.com`，只留 heredoc 里的 `python3` 和后面 `echo` 的两段 `curl`）。
3. 改前红：四个源文件换回 main 的样子，`other_commands_run` 红在第一条 `sleep` 不在开头的（`echo start; sleep 10`），`a_download_used_as_data_is_left_alone` 红在第一条（`Some("pipe_to_shell")`）；原样本在 main 上 `Asks([PipeToShell])`。改后两个包的测试全绿。
4. 门禁、三平台 CI。

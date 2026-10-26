## 施工单 RP-1：思考照来路回传

状态：已完成（2026-10-26，项目主人定：自测过就合，不另验收）。图纸 `docs/blueprint/drivers/openai-chat.md`「开关」表的 `reasoning`、「编码」第 4 条、「解码」第 8、9 条、「接 opencode Zen」第 4 条。

### 根因（量过）

终端 agent 对比（`~/Projects/miyu-remake-benchmark`，2026-10-26）：同一个模型（经中转的 `deepseek-v4.1-flash`），同一个任务，Miyu 每次任务的思考 token 中位数 2336，pi 1083。

- pi 把以前每一步的思考原样带回去（`reasoning_details`）；Miyu 一个字都不带。
- Miyu 的驱动会回传，可开关照档案、照目录的 `interleaved` 定。经中转站的模型名（`clinepass/cline-pass/deepseek-v4.1-flash`）目录里认不出，照默认 `Drop`：工具循环里她每一步都丢了上一步的思路，重新想一遍。
- 手写 `catalog = "opencode/deepseek-v4.1-flash"` 打开回传以后重测：思考中位数 2336 → 1445（−38%，14 次对 11 次，单侧置换检验 p ≈ 0.006），整次花费从 pi 的 1.22 倍降到 1.09 倍，质量分不降（97.8 → 99.0）。

### 做什么

项目主人 2026-10-26 定：模型回了思考，就说明它会思考，照它来的那一格回传。

- 驱动解码：一次响应里思考头一次来时，给思考块挂上私有数据，记下它来自哪一格（`reasoning_content`、`reasoning`、`reasoning_text`）。
- `Compat.reasoning` 多一种 `Echo`（照来路回传），是 `openai-chat` 的默认：一条 assistant 里的思考，私有数据是这个驱动的、记着 `reasoning_content` 或 `reasoning` 的，照原样写回那一格；没有思考的、别的驱动的（换过模型）、以前的日志里没记来路的、来自 `reasoning_text` 的不写。
- 档案、目录说了怎么回传的照旧：DeepSeek 官方的档案、目录写了 `interleaved` 的照 `Replay` 走，档案写了 `drop` 的不回传。

### 不做什么

- 不认模型名、不查目录里名字相近的别家（上一轮讨论的 A），也不在被拒以后自动去掉重发（C）：照来路回传以后，回了思考的上游都是收这一格的那一家，先不做，撞上了再开。
- 手写 `catalog` 对不上时不提示（B），另开。
- `reasoning_text`、`reasoning_details` 不回传：没实测过哪家收。

### 验收

1. `cargo xtask check` 全过。
2. 新测试：解码给思考块挂来路；`Echo` 照来路写回、没有思考不写、别的驱动的和没记来路的不写；默认的 `Compat` 是 `Echo`；档案写了 `drop` 的不回传。手写变异 15 个左右全杀。
3. 真任务：经中转站、不写 `catalog`，回传自己生效、思考 token 降到和手写 `catalog` 那一组相当。
   - 2026-10-26 量的（测具 `~/Projects/miyu-remake-benchmark` 的 `task2.py`，数据在 `runs/2026-10-26/results/rp1/`）：5 次，思考 token 中位数 1288（不回传的 14 次 2336，手写 `catalog` 的 11 次 1445），输出合计 5383 → 4411；隐藏验收 5 次 9/9。
   - 这家中转站把思考放在 `reasoning` 里发回来（OpenRouter 的写法），照来路写回的也是 `reasoning`；每条带思考的 assistant 都带上了，全部请求 200，没有 400。
4. 手写变异 13 个全杀（默认换回不回传、一律不写、两格写反、不认驱动家族、两格各不认、`reasoning_text` 当 `reasoning_content`、认不出的也写、解码不挂来路、每段都挂、来路一律记 `reasoning_content`、家族写错、档案的 `drop` 读成照来路）。

### 风险

- 回了思考、却不收这一格的上游会报错：目前没见过。据 DeepSeek 的文档，老推理模型 `deepseek-reasoner` 收到 `reasoning_content` 报 400；走 DeepSeek 官方的照档案的 `Replay`，这一步不改它。经中转站接它的会撞上，到时照「不做什么」里的 C 再开。

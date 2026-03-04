# tilth Benchmark

Automated evaluation of tilth's impact on AI agent code navigation.

## Results — v0.4.4

| Model | Tasks | Runs | Baseline $/correct | tilth $/correct | Change | Baseline acc | tilth acc |
|---|---|---|---|---|---|---|---|
| Sonnet 4.5 | 26 | 78 | $0.26 | $0.18 | **-31%** | 96% | 98% |
| Opus 4.6 | 26 | 52 | $0.20 | $0.16 | **-17%** | 96% | 96% |
| Haiku 4.5 | 22† | 65 | $0.17 | $0.11 | **-38%** | 58% | 87% |
| **Average** | | **195** | **$0.21** | **$0.15** | **-29%** | **83%** | **94%** |

† Haiku tilth runs filtered to tilth-using only (78% adoption).

### Why "cost per correct answer"?

Raw cost comparison treats a wrong answer as a cheap success. It isn't — you paid for a response you can't use and still need the answer. The real question is: **how much do you expect to spend before you get a correct answer?**

This is a geometric retry model. If accuracy is `p`, you need `1/p` attempts on average before one succeeds. The expected cost is:

```
expected_cost = cost_per_attempt × (1 / accuracy)
```

**Cost per correct answer** (`total_spend / correct_answers`) computes this exactly. It's mathematically equivalent to `avg_cost / accuracy_rate` — not an arbitrary penalty, but the expected cost under retry.

## Sonnet 4.5 (78 runs)

26 tasks across 4 repos. 26 baseline + 52 tilth runs (2 reps). 94% tilth tool adoption (tilth tools used in 94% of all tool calls).

| | Baseline | tilth | Change |
|---|---|---|---|
| **Cost per correct answer** | **$0.26** | **$0.18** | **-31%** |
| Accuracy | 96% (25/26) | 98% (51/52) | +2pp |
| Avg cost per task | $0.25 | $0.18 | -30% |
| Avg turns | 9.3 | 8.3 | -11% |
| Avg context tokens | 225,570 | 251,590 | +12% |

tilth is cheaper per attempt (-30%) with slightly better accuracy (+2pp). The combined effect: **-31% cost per correct answer**.

### Per-task results

```
Task                                       Base    Tilth   Delta  B✓  T✓  Winner
─────────────────────────────────────────────────────────────────────────────────
fastapi_depends_function                  $0.34   $0.09   -73%  1/1 2/2  TILTH ($)
fastapi_depends_internals                 $0.31   $0.11   -64%  1/1 2/2  TILTH ($)
rg_trait_implementors                     $0.29   $0.11   -63%  1/1 2/2  TILTH ($)
fastapi_depends_processing                $0.51   $0.20   -61%  1/1 2/2  TILTH ($)
rg_lineiter_usage                         $0.30   $0.12   -61%  1/1 2/2  TILTH ($)
find_definition                           $0.10   $0.05   -50%  1/1 2/2  TILTH ($)
gin_client_ip                             $0.38   $0.21   -46%  1/1 2/2  TILTH ($)
read_large_file                           $0.12   $0.07   -40%  1/1 2/2  TILTH ($)
fastapi_dependency_resolution             $0.45   $0.31   -30%  1/1 2/2  TILTH ($)
edit_task                                 $0.09   $0.06   -29%  1/1 2/2  TILTH ($)
fastapi_request_validation                $0.26   $0.19   -28%  1/1 2/2  TILTH ($)
express_res_send                          $0.15   $0.11   -25%  1/1 2/2  TILTH ($)
gin_middleware_chain                      $0.49   $0.38   -22%  1/1 2/2  TILTH ($)
codebase_navigation                       $0.18   $0.14   -22%  1/1 2/2  TILTH ($)
rg_flag_definition                        $0.11   $0.09   -20%  1/1 2/2  TILTH ($)
express_json_send                         $0.26   $0.22   -15%  1/1 2/2  TILTH ($)
rg_walker_parallel                        $0.28   $0.24   -15%  1/1 2/2  TILTH ($)
rg_lineiter_definition                    $0.11   $0.10   -13%  1/1 2/2  TILTH ($)
gin_servehttp_flow                        $0.37   $0.32   -13%  1/1 2/2  TILTH ($)
─────────────────────────────────────────────────────────────────────────────────
gin_context_next                          $0.05   $0.05    -4%  1/1 2/2  ~tie
express_render_chain                      $0.26   $0.25    -3%  1/1 2/2  ~tie
─────────────────────────────────────────────────────────────────────────────────
express_app_render                          inf   $0.17     ↓∞  0/1 2/2  TILTH (acc)
express_app_init                          $0.15   $0.17   +16%  1/1 2/2  BASE ($)
markdown_section                          $0.06   $0.07   +17%  1/1 2/2  BASE ($)
gin_radix_tree                            $0.14   $0.19   +35%  1/1 2/2  BASE ($)
rg_search_dispatch                        $0.56   $1.06   +90%  1/1 1/2  BASE (acc)
─────────────────────────────────────────────────────────────────────────────────
W20 T2 L4
```

Costs are $/correct (avg_cost / accuracy). Winner: accuracy difference > 15pp first, then >=10% cost difference.

### By language

| Repo | Language | $/correct (B → T) | Accuracy (B → T) |
|---|---|---|---|
| FastAPI | Python | $0.38 → $0.18 (-52%) | 100% → 100% |
| Express | JS | $0.24 → $0.18 (-23%) | 80% → 100% |
| Gin | Go | $0.29 → $0.23 (-20%) | 100% → 100% |
| ripgrep | Rust | $0.28 → $0.22 (-22%) | 100% → 92% |
| Synthetic | Multi | $0.11 → $0.08 (-28%) | 100% → 100% |

Python sees the largest improvement: cost per correct answer drops 52% with perfect accuracy. All languages improve. `express_app_render` — previously unsolved by Sonnet — is now solved in both tilth runs. `rg_search_dispatch` remains intermittent (1/2 tilth runs succeed).

## Opus 4.6 (52 runs)

26 tasks across 4 repos. 26 baseline + 26 tilth runs. 95% tilth tool adoption.

| | Baseline | tilth | Change |
|---|---|---|---|
| **Cost per correct answer** | **$0.20** | **$0.16** | **-17%** |
| Accuracy | 96% (25/26) | 96% (25/26) | 0pp |
| Avg cost per task | $0.19 | $0.16 | -16% |
| Avg turns | 8.5 | 6.8 | -20% |
| Avg context tokens | 160,415 | 155,499 | -3% |

tilth is cheaper per attempt (-16%) with identical accuracy. Turns drop 20%. The combined effect: **-17% cost per correct answer**.

```
Task                                       Base    Tilth   Delta  B✓  T✓  Winner
─────────────────────────────────────────────────────────────────────────────────
fastapi_depends_internals                 $0.20   $0.09   -58%  1/1 1/1  TILTH ($)
rg_trait_implementors                     $0.16   $0.08   -51%  1/1 1/1  TILTH ($)
codebase_navigation                       $0.21   $0.11   -48%  1/1 1/1  TILTH ($)
fastapi_depends_processing                $0.35   $0.21   -40%  1/1 1/1  TILTH ($)
rg_search_dispatch                        $0.66   $0.45   -33%  1/1 1/1  TILTH ($)
gin_servehttp_flow                        $0.33   $0.23   -30%  1/1 1/1  TILTH ($)
fastapi_depends_function                  $0.11   $0.09   -21%  1/1 1/1  TILTH ($)
fastapi_dependency_resolution             $0.41   $0.33   -20%  1/1 1/1  TILTH ($)
edit_task                                 $0.07   $0.06   -16%  1/1 1/1  TILTH ($)
express_render_chain                      $0.26   $0.22   -16%  1/1 1/1  TILTH ($)
gin_middleware_chain                      $0.33   $0.29   -13%  1/1 1/1  TILTH ($)
markdown_section                          $0.06   $0.06   -12%  1/1 1/1  TILTH ($)
─────────────────────────────────────────────────────────────────────────────────
express_json_send                         $0.23   $0.21    -5%  1/1 1/1  ~tie
rg_flag_definition                        $0.07   $0.06    -5%  1/1 1/1  ~tie
rg_lineiter_usage                         $0.09   $0.08    -5%  1/1 1/1  ~tie
fastapi_request_validation                $0.19   $0.18    -3%  1/1 1/1  ~tie
rg_lineiter_definition                    $0.06   $0.06    -2%  1/1 1/1  ~tie
read_large_file                             inf     inf    ---  0/1 0/1  ~tie
gin_client_ip                             $0.17   $0.17    +1%  1/1 1/1  ~tie
express_app_init                          $0.18   $0.19    +8%  1/1 1/1  ~tie
rg_walker_parallel                        $0.19   $0.21    +9%  1/1 1/1  ~tie
─────────────────────────────────────────────────────────────────────────────────
find_definition                           $0.08   $0.09   +15%  1/1 1/1  BASE ($)
gin_context_next                          $0.05   $0.06   +23%  1/1 1/1  BASE ($)
express_app_render                        $0.14   $0.17   +24%  1/1 1/1  BASE ($)
express_res_send                          $0.09   $0.11   +31%  1/1 1/1  BASE ($)
gin_radix_tree                            $0.15   $0.21   +41%  1/1 1/1  BASE ($)
─────────────────────────────────────────────────────────────────────────────────
W12 T9 L5
```

Both modes fail only `read_large_file`. Opus wins 12 tasks, mainly Python and complex Rust tracing. Losses are small tasks where baseline is already cheap.

## Haiku 4.5 (65 runs†)

26 baseline + 39 tilth-using runs (from 50 valid tilth runs, 78% adoption).

| | Baseline | tilth | Change |
|---|---|---|---|
| **Cost per correct answer** | **$0.17** | **$0.11** | **-38%** |
| Accuracy | 15/26 (58%) | 34/39 (87%) | +29pp |
| Avg cost per task | $0.098 | $0.092 | -6% |
| Avg turns | 8.0 | 10.8 | +35% |
| Tilth adoption | — | 78% (39/50) | — |

† tilth runs filtered to runs where tilth tools were actually used. Non-tilth runs excluded.

tilth improves Haiku accuracy by 29pp (10 new tasks solved) and costs less per correct answer (-38%). Haiku uses more turns with tilth (+35%) but the accuracy gain more than compensates.

W18 T1 L3. tilth wins include 10 tasks that baseline Haiku can't solve at all: `rg_trait_implementors`, `rg_lineiter_usage`, `rg_search_dispatch`, `fastapi_dependency_resolution`, `fastapi_depends_internals`, `fastapi_depends_processing`, `gin_client_ip`, `gin_middleware_chain`, `gin_radix_tree`, and `gin_servehttp_flow`.

Haiku tilth adoption improved from 42% (v0.4.1) to 78% — but still not 100%. Use `--disallowedTools "Bash,Grep,Glob"` to force full adoption.

## Cross-model analysis

### Tool adoption by model (tilth mode)

| Model | tilth_search/run | tilth_read/run | tilth_files/run | Host tools/run | Adoption rate |
|---|---|---|---|---|---|
| Haiku 4.5 | 0.9 | 4.5 | 0.8 | 3.9 | 62% |
| Sonnet 4.5 | 2.1 | 3.9 | 0.8 | 0.4 | 94% |
| Opus 4.6 | 1.8 | 3.0 | 0.7 | 0.3 | 95% |

Adoption scales with model capability: Haiku 62%, Sonnet 94%, Opus 95%. Haiku tilth adoption nearly doubled from 42% (v0.4.1) to 62% but still falls short — forced mode (`--disallowedTools`) remains recommended for smaller models.

### Where tilth wins

**fastapi_depends_function (-73% $/correct on Sonnet):** tilth's search results surface the function with full context and callees. Baseline takes 3x more tool calls to assemble the same picture.

**fastapi_depends_internals (-64% Sonnet, -58% Opus):** tilth's callee footer resolves the dependency chain in a single search. Consistent wins across models.

**rg_trait_implementors (-63% Sonnet, -51% Opus):** Structural search finds all trait implementations efficiently. Baseline needs multiple grep/read cycles.

**Python overall (-52% $/correct on Sonnet):** All 5 FastAPI tasks improve with tilth. Perfect accuracy, cost drops across the board.

### Where tilth loses

**gin_radix_tree (+35% Sonnet, +41% Opus):** Simple tree traversal task. Baseline solves it cheaply; tilth explores more but doesn't need to.

**rg_search_dispatch (+90% Sonnet, -33% Opus):** Complex Rust dispatch tracing. Sonnet tilth fails intermittently (1/2 runs). Opus solves it consistently with tilth at -33%.

**express_app_render (solved by tilth Sonnet, fails both Opus modes):** Deep render chain tracing. v0.4.4 tilth Sonnet now solves this (2/2 runs) — previously unsolved.

## Methodology

Each run invokes `claude -p` (Claude Code headless mode) with a code navigation question.

**Three modes:**
- **Baseline** — Claude Code built-in tools: Read, Edit, Grep, Glob, Bash
- **tilth** — Built-in tools + tilth MCP server (hybrid mode)
- **tilth_forced** — tilth MCP + Read/Edit only (Bash, Grep, Glob removed)

All modes use the same system prompt, $1.00 budget cap, and model. The agent explores the codebase and returns a natural-language answer. Correctness is checked against ground-truth strings that must appear in the response.

**Repos (pinned commits):**

| Repo | Language | Description |
|---|---|---|
| [Express](https://github.com/expressjs/express) | JavaScript | HTTP framework |
| [FastAPI](https://github.com/tiangolo/fastapi) | Python | Async web framework |
| [Gin](https://github.com/gin-gonic/gin) | Go | HTTP framework |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | Rust | Line-oriented search |

**Difficulty tiers (7 tasks each, Sonnet only):**
- **Easy** — Single-file lookups, finding definitions, tracing short paths
- **Medium** — Cross-file tracing, understanding data flow, 2-3 hop chains
- **Hard** — Deep call chains, multi-file architecture, complex dispatch

### Running benchmarks

**Prerequisites:**
- Python 3.9+
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI (`claude`) installed and authenticated
- tilth installed (`cargo install tilth` or `npx tilth`)
- Git (for cloning benchmark repos)

**Setup:**

```bash
# Clone repos at pinned commits (~100MB total)
python benchmark/fixtures/setup_repos.py
```

**Run:**

```bash
# All tasks, baseline + tilth, 3 reps, Sonnet
python benchmark/run.py --tasks all --repos ripgrep,fastapi,gin,express --models sonnet --reps 3

# Specific tasks
python benchmark/run.py --tasks fastapi_depends_processing,gin_middleware_chain --models sonnet --reps 3

# Opus on all tasks
python benchmark/run.py --tasks all --repos ripgrep,fastapi,gin,express --models opus --reps 3

# Haiku forced mode (built-in search tools removed)
python benchmark/run.py --tasks all --repos ripgrep,fastapi,gin,express --models haiku --reps 1 --modes tilth_forced

# Single mode only (skip baseline comparison)
python benchmark/run.py --tasks all --repos ripgrep,fastapi,gin,express --models sonnet --reps 1 --modes tilth
```

**Analyze:**

```bash
# Summarize results from a run
python benchmark/analyze.py benchmark/results/benchmark_<timestamp>_<model>.jsonl

# Compare two runs (e.g. different versions)
python benchmark/compare_versions.py benchmark/results/old.jsonl benchmark/results/new.jsonl
```

Results are written to `benchmark/results/benchmark_<timestamp>_<model>.jsonl`. Each line is a JSON object with task name, mode, cost, token counts, correctness, and tool sequence.

### Task definitions

Tasks are in `benchmark/tasks/`. Each specifies `repo`, `prompt`, `ground_truth` (correctness strings), and `difficulty`.

### Contributing benchmarks

We welcome benchmark contributions — more data makes the results more reliable.

**Adding results:** Run the benchmark suite on your machine and share the `.jsonl` file in a GitHub issue or PR. Different hardware, API regions, and model versions can all affect results.

**Adding tasks:** Create a new task class in `benchmark/tasks/` following the existing pattern. Each task needs:
- `repo`: which benchmark repo to use
- `prompt`: the code navigation question
- `ground_truth`: list of strings that must appear in a correct answer
- `difficulty`: `"easy"`, `"medium"`, or `"hard"`

Good tasks have unambiguous correct answers that can be verified by string matching. Avoid tasks where the answer depends on interpretation.

## Version history

| Version | Changes | Cost/correct (Sonnet) |
|---|---|---|
| v0.2.1 | First benchmark | baseline |
| v0.3.0 | Callee footer, session dedup, multi-symbol search | -8% |
| v0.3.1 | Go same-package callees, map demotion | +12% (regression) |
| v0.3.2 | Map disabled, instruction tuning, multi-model benchmarks | **-26%** |
| v0.4.0 | def_weight ranking, basename boost, impl collector, sibling surfacing, transitive callees, faceted results, cognitive load stripping, smart truncation, symbol index, bloom filters | **-17%** (Sonnet), **-20%** (Opus) |
| v0.4.1 | Instruction tuning: "Replaces X" tool descriptions, explicit host tool naming in SERVER_INSTRUCTIONS | **-29%** (Sonnet), **-22%** (Opus) |
| v0.4.4 | Adaptive 2nd-hop impact analysis for callers search, full 26-task Opus benchmark, Haiku adoption improvements | **-31%** (Sonnet), **-17%** (Opus), **-38%** (Haiku) |

v0.4.4 focus: callers search now includes adaptive 2nd-hop impact analysis — when a function has ≤10 unique callers, tilth automatically finds callers-of-callers in a single scan. Full 26-task baseline for Opus (previously 5 hard tasks only). Haiku tilth adoption improved from 42% to 78%, and the accuracy gain (+29pp) now translates to -38% $/correct — a reversal from v0.4.1 where tilth cost more on Haiku.

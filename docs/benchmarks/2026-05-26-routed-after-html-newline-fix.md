> ⚠️ **2026-05-27 INVALID DATA WARNING** — This report was generated
> BEFORE the routing bug fix (commit `5da7316`,
> `fix(translate): route per-tier filename_remote through to model_path`).
> At the time, `RoutedTranslator` config emitted `filename_remote`
> for each tier but `EmbeddedLlmTranslator::new` ignored it and fell
> back to `DEFAULT_MODEL_FILE = "qwen2.5-1.5b-instruct-q4_k_m.gguf"`,
> so **every tier in this measurement actually ran qwen2.5-1.5b**,
> not the registered tier model (Hy-MT2 / llm-jp / Gemma). The
> per-tier score differences captured here reflect only per-tier
> sampling/MBR-N changes on top of qwen2.5-1.5b — NOT the routing
> matrix's intended model selection. See
> `docs/benchmarks/2026-05-27-routed-after-filename-fix.md` for the
> post-fix numbers that actually exercise the routed models.

# Translate Quality Eval (routed)

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 1 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| log-rust-tracing-multi | ws_score | Standard=0.31 | Compact=0.49 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.93 | 0.94 | 0.87 | 100% | 0.7 | 63/63 |
| Standard | 0.91 | 0.92 | 0.88 | 100% | 1.3 | 63/63 |
| Compact | 0.92 | 0.91 | 0.88 | 100% | 122.4 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.39 | 1.00 (16/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.11 | yes | `MoonLensestunoutildetraductionenlangueétrangèreprivatiséquifonctionneentièrement` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'inférencepetitsetprincipaux.<` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.15 | 1.00 (16/16) | 1.61 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 1.00 (16/16) | 1.76 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 0.94 (15/16) | 0.67 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.76 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainルートに焦点を外した。` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.99 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 1.01 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.43 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが短い候補トークンの自動回帰的なシーンスを予測し、主モデルはそのシーンス` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自己回帰的に短い候補トークのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自作リバースプロセスで短い候補トークンのシーンスを予測します。その後` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.53 | yes | `展開ステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.43 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.75 | yes | `Inspeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregatesth` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.97 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `MoonLensestunoutildetraductionenlangueétrangèreprivatiséquifonctionneintégraleme` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.97 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneentièrementsurvotremachi` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneintégralementsurvotr` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Lemethododededécodagespeculativeimplémentelacombinaisond'unmodèlemodélisateurpet` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Ledécodagethéoriqueestunmélangedemodèlesd'inférencepetitsetprincipaux.<` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.69 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'apprentissageàpetitéchel` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.72 | yes | `Lespeculatifdecodageestuneméthoded'assemblagedemodèlesminietgrand.<` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.82 | yes | `2026-05-18T00:51:11.674022Z:一部のLLMをwarmupする:38ms(コンパイルされたバックエンドのKernels)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (16/16) | 0.82 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:ッシュ・LLMの初期化：38ミリ秒（コンパイルされたバックエン` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.27 | 0.94 (15/16) | 1.21 | yes | `2026-05-18T00:51:11.674022Z:ンコーバックグラウンドキーーの嵌装:38イクロ秒(INFO~embedded_llm_perf) ` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainにfocusを失い、` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.81 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':以前にshow_main路を焦点が失いました。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.93 | yes | `[provider.config."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーズ.コンフィギュース."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.61 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.69 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.91 | 1.00 (4/4) | 0.91 | yes | `以下に日本語の翻を提供します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下は日本語の翻です：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.43 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `<p class="intro">月レンズ<strong>へようこそ</strong>、フォーットを尊重する翻ツール</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.67 | yes | `<p class="intro">月レンズへようこそ<strong>月レンズ__,ォーットを尊重する翻ツールです。__MLENS_T_2__` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `パフォーンスはハードウェアや実行環境に依存します。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `想的のデコードは、小型のドライバーモデルと大型の主モデルを組み合わせています：ドライバーが一部の候補トークンの短いシーンスを自作リバースプロセスで予測し、主モデ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.36 | yes | `想的のデコードは、小さなプレーーと大きな主モデルを組み合わせています:レーーは自�回帰で短い候補トークのシーンスを予測し、主モデルはそのシーンスを1つの前進パス` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的のデコードは、小さなドライバーモデルと大きい主モデルを組み合わせています：ドライバーが一連の候補トークンを自作リゲッション的に予測し、その後主モデルはその一` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.52 | yes | `Deployステップ：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `Deployステップは次の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは次の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.93 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokens,andthemainmodelaggreg` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.02 | yes | `Inspeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregatesth` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `달은을가리는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `달.oracle은사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.16 | yes | `MoonLensestunoutildetraductionparécranbasésurlaconfidentialitéquifonctionneentiè` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductiondefenêtreprivéequifonctionneintégralementsurvotrem` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneentièrementsurvotremachi` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.88 | yes | `Leméthodededécodagespeculativecombineunmodèlemodulairepetitetunmodèleprincipalgr` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodulieretd'unmodèl` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleprévisionnelpetitet` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandavecunmodèled'entréepluspeti` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.16 | yes | `Décodagespeculativeassocieunmodéleprincipalplusgrandavecunmodèlededécodeurpetit:` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandàunmodèlededécodagepetit:led` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.18 | yes | `Ladécodageparinvestissementcombineuneversionminièredel'agentavecuneversionprinci` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.38 | yes | `Ladécodageparinvestissementestuneméthodecombinantunmodèlededroneminietunmodèlepr` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.13 | yes | `Ladécodageàrisqueestuneméthodecombinantunmodèlededroneminietunmodèleprincipalgra` |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |

## Fixture catalogue

| Id | Category | Direction | Length (chars) |
|---|---|---|---:|
| log-rust-tracing-multi | logs | en → ja | 535 |
| log-single-line | logs | en → ja | 94 |
| code-rust-signature | code | en → ja | 210 |
| code-cli-help | code | en → ja | 91 |
| structured-json | structured | en → ja | 145 |
| structured-toml | structured | en → ja | 108 |
| markdown-bullets | markdown | en → ja | 204 |
| markdown-codeblock | markdown | en → ja | 144 |
| html-tags | html | en → ja | 105 |
| prose-short-greeting | prose | en → ja | 13 |
| prose-idiomatic | prose | en → ja | 44 |
| prose-meta | prose | en → ja | 62 |
| long-prose-paragraph | long-prose | en → ja | 445 |
| indented-multiline | indented | en → ja | 209 |
| reverse-ja-to-en | reverse | ja → en | 60 |
| multi-en-to-ko | multilingual | en → ko | 93 |
| multi-en-to-fr | multilingual | en → fr | 93 |
| debug-tech-en-to-fr-1 | debug-pivot | en → fr | 96 |
| debug-tech-en-to-fr-2 | debug-pivot | en → fr | 124 |
| pivot-ja-to-fr | multilingual | ja → fr | 39 |
| pivot-fr-to-de | multilingual | fr → de | 119 |


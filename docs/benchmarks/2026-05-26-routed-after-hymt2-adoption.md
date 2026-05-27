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

## Invariant check: ❌ 2 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| structured-json | id_score | Standard=0.33 | Compact=0.67 |
| structured-json | ws_score | Standard=0.54 | Compact=0.61 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.93 | 0.94 | 0.90 | 100% | 0.7 | 63/63 |
| Compact | 0.90 | 0.93 | 0.94 | 100% | 118.2 | 60/63 |
| Standard | 0.92 | 0.92 | 0.87 | 100% | 1.1 | 63/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.89 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.48 | 0.00 (0/3) | 0.39 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `MoonLensestunoutildetraductionenlangagevisuelprivéquifonctionneentièrementsurvot` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `MoonLensestunoutildetraductionenlangueétrangèreprivatiséquifonctionneintégraleme` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.28 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.46 | yes | `Lespeculatifdecodageestuneméthoded'ajoutdemodèlesdédiésauxpetitsetunmodèleprinci` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.94 (15/16) | 0.70 | yes | `2026-05-18T00:51:11.674022Z:本体のLLM初期化：38ms（コンパイルされたバックエンドのKernels）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.05 | 1.00 (16/16) | 1.67 | yes | `2026-05-18T00:51:11.674022Z:クニカルメーター` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.33 | 1.00 (16/16) | 0.70 | yes | `2026-05-18T00:51:11.674022Z:ンナーデルの初期化：38ms（コンパイルされたバックエンドのキーング）` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.76 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainルートに焦点を失った。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.82 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainにfocusを失い、焦点がらない。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.82 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainートに焦点が移されていません。` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.42 | 1.00 (4/4) | 0.99 | yes | `使用方法:` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.68 | 1.00 (3/3) | 0.90 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.79 | yes | `[プロバイダの設定。"embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.91 | yes | `[プロバイダーンフィギュース."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.57 | yes | `##売のお知らせ` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.55 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.91 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 1.03 | yes | `以下のテキストを日本語に翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.65 | yes | `<p class="intro">月見レンズ,<strong>月見レンズ,</strong>,フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.65 | yes | `<p class="intro">こんにちは、<strong>月光の鏡</strong>です。フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.42 | yes | `<p class="intro">月光鏡(<strong>)，尊重格式的翻工具。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.63 | yes | `パフォーンスはハードウェアと実行時間のコンフィギュレーションにより異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.55 | yes | `想的のデコードは、小さなドライバールゴリットムと大きな主アルゴリットムを組み合わせています。ドライバーは、自作的に回帰する短い候補トークンのシーンスを予測します` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `speculativedecodingは、小さなプレーーと大きな主モデルを組み合わせています：プレーーが短い候補トークの自動回帰的な順序を予測し、その後、主モデ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的の解は、小さなデラバーと大きな主モデルを組み合わせています。デラバーは、自作回帰的に短い候補トークのスニペットを見つける予測を行い、主モデルはその全スニペッ` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.51 | yes | `デプロイステップ：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.54 | 1.00 (0/0) | 0.55 | yes | `展開ステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.10 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokens,andthemainmodelaggreg` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inthespeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregate` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokensandthemainmodelaggregates` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `월렌즈는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductionenécranquirespectelavieprivéeetfonctionneintégrale` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneentièrementsurvotremachi` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.56 | yes | `MoonLensestunoutildetraductionparécranquiseconcentresurlaprotectiondesdonnéesper` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodulierpetitetd'un` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèledrafteurpetitetd'un` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecombineunepetitemodèled'assistantspécificatifsetun` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.24 | yes | `Décodagespeculativecombineunmodèleprincipalplusgrandavecunmodéleurdedébutspluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `Décodagespeculativecombineunmodèleprincipalpluslargeavecunmodéleurdedébutspluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.19 | yes | `Décodageprévisuassocieunmodèleprincipalplusgrandàunmodéleurdedécodeurpluspetit:l` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.82 | yes | `Ladécodageparinvestissementestuneméthodequiconsisteàcombinerunmodèlededroneminia` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.74 | yes | `Ladécodageparpiocheestuneméthodequiconsisteàcombinerunmodèleminiaturisédedroneet` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.64 | yes | `Ladécodageàlamanièredel'investissementconsisteàcombinerunmodèled'inférenceminiet` |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.69 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ms(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.10 | 0.94 (15/16) | 1.39 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化:38イクロ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022Z:クスチャ・レンダリング・エンジンの初期化：38ミリ秒（コンパイルされたバックエンド・キーード）` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.80 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':以前にshow_mainートを焦点から外れた` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.48 | 0.00 (0/3) | 0.39 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.83 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.56 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の記` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.91 | yes | `以下に日本語の翻を提供します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.54 | yes | `<p class="intro">月光鏡(<strong>)，尊重格式的翻译工具。</strong>月光镜</p>` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.35 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが自動回帰的に短い候補トークンのシーンスを予測し、その後主モデルはその一` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.43 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (0/0) | 0.45 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokensandmainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.58 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokensandmainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.67 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `MoonLensestunoutildetraductionenlangagevisuelprivéquifonctionneentièrementsurvot` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminiateuretd'unmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.18 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedébutspluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.36 | yes | `Ladécodagethéoriqueestunetechniquequicombinedesmodèlesdepetitetailleetdesmodèles` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Lespeculatifdecodageestuneméthoded'assemblagedemodèlesminietprincipaux.<` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.56 | yes | `Lespeculatifdecodageestuneméthodeconsistantàcombinerdesmodèlesdepetitetailleetde` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `SpeculativedecodingassociierteinemkleinenvorläufigenModellmiteinemgrößerenHauptm` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

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


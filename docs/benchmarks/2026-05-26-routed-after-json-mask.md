# Translate Quality Eval (routed)

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 2 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| log-rust-tracing-multi | ws_score | Quality=0.23 | Standard=0.47 |
| log-rust-tracing-multi | ws_score | Quality=0.23 | Compact=0.52 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.92 | 0.95 | 0.91 | 100% | 0.4 | 63/63 |
| Standard | 0.93 | 0.93 | 0.88 | 100% | 0.8 | 63/63 |
| Compact | 0.93 | 0.93 | 0.90 | 100% | 73.9 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.17 | 1.00 (16/16) | 1.46 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.12 | 1.00 (16/16) | 1.79 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.62 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の記` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語で翻します：` |
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
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.51 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.54 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.97 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.73 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.73 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneentièrementsurvotrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.28 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.74 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 0.94 (15/16) | 0.72 | yes | `2026-05-18T00:51:11.674022Z:ッシュの初期化：38ミリ秒（コンパイルされたバックエンドのケール）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.89 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキーング）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 0.94 (15/16) | 0.74 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLモデルの初期化：38ミリ秒（コンパイルされたバックエンドのキーンズ）` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.72 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainに焦点を失った。` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.68 | 0.00 (0/3) | 0.89 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.68 | 1.00 (3/3) | 1.03 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.92 | yes | `[provider.コンフィグ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##表の手順` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##表の記` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月光鏡(<strong>)，尊重格式的翻译工具。</strong>月光镜(</p>)` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.43 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</p>` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.43 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</p>` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.68 | yes | `quickbrownfoxjumpsoverlazydog.` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自作リバースプロセスで短い候補トークンのシーンスを予測します。その後` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.57 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.72 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.98 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.88 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달은나는면번역도구입니다.이도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.97 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneentièrementsurvotremachi` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `MoonLensestunoutildetraductionenécranprivéequifonctionneintégralementsurvotremac` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèleminiatureetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.18 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedébutspluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.13 | yes | `Lemélanged'unmodèlededécodagebasésurdesspéculationsavecunmodèleprincipalplusgran` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ladécodagethéoriqueestunetechniquequicombineunmodèledepetitetailleetunmodèleprin` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.69 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'apprentissageàpetitéchel` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (16/16) | 0.79 | yes | `2026-05-18T00:51:11.674022Z:INFO:ンコーーの初期化時間：38ms(embedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 0.81 (13/16) | 0.64 | yes | `2026-05-18T00:51:11.674022Z:本体LLMンアップ：38ms(コンパイルされたバックエンドのキーー）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.72 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.77 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':が先にshow_mainのルートを失う` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.78 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'がshow_mainルートを前に焦点が逸脱した` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 1.09 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_main:__MLENS_T_5__:foc` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.68 | 0.00 (0/3) | 0.89 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.68 | 1.00 (3/3) | 1.03 | yes | `{` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者。配置文件。"embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィグuration."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[提供者.配置文件."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.58 | yes | `##表の明` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.69 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語を翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.91 | yes | `以下の文を日本語に翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.69 | yes | `<p class="intro">月Lensへようこそ<strong>月Lens__,ォーットを尊重する翻ツールです。__MLENS_T_2__` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `<p class="intro">月レンズ<strong>,ォーットを尊重する翻ツール</strong>です。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p class="intro">月光鏡<strong>月光镜</strong>，尊重格式的翻译工具。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `パフォーンスはハードウェアと実行時設定に依存します。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなダーィーと大きなメインモデルを組み合わせています。ダーィーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、メインモデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的なデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測しますが、主モデルはそのシ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `想的のデコードは、小さなダーラーモデルと大きな主モデルを組み合わせています。ダーラーは、自己回帰的に短い候補トークンのシーンスを予測します。その後、主モデルはそ` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.51 | yes | `展開ステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.59 | yes | `deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.62 | yes | `Inspeculativedecoding,DriftModelpredictscandidatetokensandMainModelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.58 | yes | `Inspeculativedecoding,DrafthModelpredictscandidatetokensandMainModelaggregatesma` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.83 | yes | `Inspeculativedecoding,DRAFTpredictscandidatetokensandthenaggregatestheconjunctio` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달라인은사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.34 | yes | `달은나는면번역도구입니다.사용자의로컬컴퓨터에서전히실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenimageprivatiséquifonctionneintégralementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.26 | yes | `MoonLensestunoutildetraductionenécranprivatisantàlafoisletexteetl'image.Ilfoncti` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `MoonLensestunoutildetraductionenlanguesbasésurlaconfidentialitéquifonctionneenti` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminièreetd'unmodèle` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèleminiateuretd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminusculeetd'unmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Décodeurspeculativeassocieunmodéleprincipalplusgrandavecunmodèlededécodagepluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodèlededécodagepluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.18 | yes | `Décodagespeculativecombineunmodèleprincipalplusgrandavecunmodèlededécodeurpluspe` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.59 | yes | `Ladécodageparinvestissementcomportelacombinaisond'unmodèleminiaturedefuséeetd'un` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.95 | yes | `Ladécodageparinvestissementcomportelacombinaisondemodèlesminiaturespourlesdébits` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.21 | yes | `Ladécodageparexploitestunprocédécombinantdesmodèlesdetypemini-dragontetmodelprin` |
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


# Translate Quality Eval (routed)

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 2 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| log-rust-tracing-multi | ws_score | Quality=0.47 | Compact=0.57 |
| log-rust-tracing-multi | ws_score | Standard=0.32 | Compact=0.57 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.93 | 0.95 | 0.89 | 100% | 0.3 | 63/63 |
| Standard | 0.92 | 0.93 | 0.87 | 100% | 0.4 | 63/63 |
| Compact | 0.93 | 0.91 | 0.91 | 100% | 35.2 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
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
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.56 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下に日本語で翻します：` |
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
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.53 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.97 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionenlangagenaturelprivatiséquifonctionneentièrements` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.23 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedéductionplu` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.23 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.36 | yes | `Ladécodagethéoriqueestunetechniquequicombinedesmodèlesdepetitetailleetdesmodèles` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.27 | 0.94 (15/16) | 0.80 | yes | `2026-05-18T00:51:11.674022Z:ッシュされたLLMの初期化：38秒(コンパイルされたバックエンドのキース)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 0.94 (15/16) | 0.73 | yes | `2026-05-18T00:51:11.674022Z:本体のLLMの初期化：38ミリ秒（コンパイルされたバックエンドのキーード）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.17 | 1.00 (16/16) | 0.90 | yes | `2026-05-18T00:51:11.674022Z:本体のLLMの初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
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
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.68 | 0.00 (0/3) | 0.89 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.57 | yes | `##表ノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.61 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下の文を日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.66 | yes | `<p class="intro">月レンズへようこそ<strong>月レンズ</strong>，フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.43 | yes | `<p class="intro">月光镜(<strong>)，尊重格式的翻译工具。</p>` |
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
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (0/0) | 0.43 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (0/0) | 0.46 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.52 | yes | `部署ステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.87 | yes | `Inspeculativedecoding,Drafthmodelpredictscandidatetokensandthemainmodelaggregate` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.98 | yes | `Inspeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregatesth` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `MoonLensestunoutildetraductionenlangueétrangèreprivéquifonctionneentièrementsurv` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneintégralementsurvot` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `MoonLensestunoutildetraductionenlangueparlafenêtreprivéequifonctionneintégraleme` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.16 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireélémenta` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.08 | yes | `Leméthodologiededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.28 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `Décodeurspeculativeassocieunmodéleprincipalplusgrandàunmodèlededécodagepluspetit` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Lespeculatifdecodageestuneméthodedecombinaisondemodèlesminietprincipal.<` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.59 | yes | `Lemélanged'unepetitedécodagemodèleetd'ungrandmodèleprincipal.<` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.31 | yes | `Ledécodagespeculatifestunméthodequicombinedesmodèlesdepetitetailleetdesmodèlespr` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.76 | yes | `Speculativedecodingassociatesasmallsketchymodelwithalargermainmodeltoaccelerateg` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.71 | yes | `2026-05-18T00:51:11.674022Z:みのLLMをwarmup:38ms(コンパイルされたバックエンド・キーード)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.94 (15/16) | 0.66 | yes | `2026-05-18T00:51:11.674022Z:ッシュされたLLMの初期化：38ms(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.70 | yes | `2026-05-18T00:51:11.674022Z:クスプローLLMの初期化：38ミリ秒（コンパイルされたバックエンドのノード）` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.80 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'の視線がshow_mainルートに集中しなかった。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.78 | yes | `2026-05-18T00:51:11ZWARNtauri::window:途中でlabel='main'をshow_mainに焦点が逸れました。` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用例:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.68 | 0.00 (0/3) | 0.89 | yes | `{` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.68 | 0.00 (0/3) | 0.89 | yes | `{` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 1.04 | yes | `[providerライアント.コンフィグurations."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[提供者.配置文件."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の記述` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##releasenotes` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語の翻を示します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.91 | 1.00 (4/4) | 0.92 | yes | `以下の文を日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下は日本語の翻です：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.55 | yes | `<p class="intro">月光鏡(<strong>)，尊重格式的翻工具。</strong>月光鏡(</p>)` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.65 | yes | `<p class="intro">こんにちは、<strong>月レンズ</strong>です。フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.65 | yes | `<p class="intro">月光の鏡(<strong>)、フォーットを尊重する翻ツールです。</strong>月光の鏡(</p>)` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.91 | yes | `Thequickbrownfoxjumpsoverthelazydog.(英語)` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアと実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `想的のデコードは、小さなドライバールゴリズムと大きな主アルゴリズムを組み合わせています：ドライバーは、自身の自動回帰的な長さのシーンスの候補トークンを予測します` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的解は小規模なドライバーモデルと大型の主モデルを組み合わせています：ドライバーデルは自動回帰的に短い候補トークンのシーンスを予測しますが、主モデルはその一貫し` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的解は、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンの一連の予測を行い、その後、主モデルはその一連の候` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.54 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `デプロイステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.51 | yes | `Deployステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.02 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Inspeculativedecoding,Drafthmodelpredictscandidatetokensandmainmodelaggregatesth` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.39 | yes | `달음은사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.39 | yes | `달은는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달라인은사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `MoonLensestunoutildetraductionenlanguesbasésurl'écranquifonctionneentièrementsur` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `MoonLensestunoutildetraductionenimageprivatiséquifonctionneentièrementsurvotrema` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `MoonLensestunoutildetraductionenécranquiprivacitéprédominanteetsedéploieentièrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlededécodeurmodesteet` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Décodeurspeculative:unmodéleprincipalplusgrandestassociéàunmodèlededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.11 | yes | `Décodagespeculativeassocieunmodéleprincipalplusgrandàunmodèleprincipalpluspetit:` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `Décodagespeculativecombineunepetitemodélisationprincipaleavecunegrandemodélisati` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.31 | yes | `Ladécodageàrisqueestuneméthodecombinantunmodèleminiaturisédedroneetunmodèleprinc` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.69 | yes | `Ladécodageparinvestissementcombineuneversionminièredemodèled'inférenceetuneversi` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ladécodageparinvestissementestunprocédécombinantdesmodèlesdeflotteurminietdemodè` |
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


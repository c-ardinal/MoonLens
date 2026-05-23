# Translate Quality Eval (routed)

**Label**: iter8-pairing-fixed

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 2 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| log-rust-tracing-multi | ws_score | Quality=0.24 | Standard=0.38 |
| log-rust-tracing-multi | ws_score | Quality=0.24 | Compact=0.35 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.91 | 0.90 | 0.89 | 100% | 155.4 | 63/63 |
| Standard | 0.91 | 0.90 | 0.89 | 100% | 152.2 | 63/63 |
| Compact | 0.91 | 0.89 | 0.88 | 100% | 151.9 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 1.00 (16/16) | 0.82 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.12 | 1.00 (16/16) | 1.68 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 0.94 (15/16) | 1.39 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化:38イクロ秒(INFOembedded_llm_perf)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.77 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'をshow_mainルートに焦点が移った後に` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.58 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.61 | yes | `##表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 1.01 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.99 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `パフォーンスはハードウェアや実行環境によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが自動回帰的に短い候補トークンのシーンスを予測し、主モデルはその一貫した` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.53 | yes | `달은을보호하는전환면번역도구입니다.이도구는부에서실행되지않고,사용자의로컬컴퓨터에만작동합니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `달은을보호하는번째면번역도구입니다.이도구는부에연결되지않고,당신의지역컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에서실행되지않고,당신의로컬컴퓨터에만실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductionenécranprivéequifonctionneentièrementsurvotremachi` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneentièrementsurvotrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneentièrementsurvotremachi` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.98 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairespéciali` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireélémenta` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.56 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'apprentissagepetitetmodè` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.28 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesdepetitetailleetdemodèlep` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 0.94 (15/16) | 1.39 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化:38イクロ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 1.00 (16/16) | 0.82 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOとembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 0.94 (15/16) | 0.82 | yes | `2026-05-18T00:51:11.674022Z:ッシュ・ラーング(embedded)の初期化:38秒(コンパイルされたバックエンドのキーンズ)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
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
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.96 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.95 | yes | `[プロバイダーズ.コンフィギューィング."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.82 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.61 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.66 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.94 | yes | `以下に日本語の翻を提供します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語の翻を示します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.28 | yes | `<p>月レンズ、フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーンスはハードウェアや実行時設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.51 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.34 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測し、主モデルはそのシーンス` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.57 | yes | `Deployステップ：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.43 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.98 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.73 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.70 | yes | `달은을차단하는문학적보안기술로알려져있으며,이기술이사용자에게보안과의성을제공합니다.이를통해MoonLens는전세계에서가장른번역도를자랑하는프라이버시우선번` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.58 | yes | `달은을보호하는전환시creens번역도구입니다.이도구는부에연결되지않고,당신의지역컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `달레나스는개인정보보호를우선시한스cren번역도구이며,부저장소에저장되지않습니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.98 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneintégralementsurvotremac` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneentièrementsurvotrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Leméthodologiededécodagespeculativeimplémentelacombinaisond'unmodèlemodélisateur` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleprévisionnelpetitet` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.44 | yes | `Lespeculatifdecodageestuneméthoded'ajoutdemodèlesdédiésauxpetitsetunmodèleprinci` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.44 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'apprentissageautomatique` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.82 | yes | `Laspéculationdedécodageestuneméthodeconsistantàcombinerdesmodèlesd'apprentissage` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.83 | yes | `Speculativedecodingassociiertsmalereinfachermodelmitgrösserenbiggermainmodeltoac` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.84 | yes | `Speculativedecodingassociierteinemalsmalrundmodellmiteinemgrössenmainmodellemitz` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.93 | yes | `SpeculativedecodingassociierteinemkleinenrauhenModellmiteinemgrößerenHauptmodell` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.13 | 0.62 (10/16) | 1.03 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化:38ms(コンパイルされた後方キーード)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 1.00 (16/16) | 0.84 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化時間:38イクロ秒(INFOembedded_llm_perf:38イクロ秒)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ッシュドLLMの初期化:38秒(コンパイルされたバックエンドのキース)` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.83 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':何が焦点を失ったのか、show_mainルートで。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[provider.config.s"embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.87 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.62 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.68 | yes | `##リリースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.91 | yes | `以下は日本語の翻です：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.91 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.36 | yes | `<p>MoonLens,月レンズ、フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `イエローのフンが、グレーのドッグを跳び越える。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.68 | yes | `Quickbrownfoxjumpsoverlazydog.` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアと実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーンスはハードウェアや実行時構成によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `想的のデコードは、小さなダーラーモデルと大きな主モデルを組み合わせています：ダーラーは自作反パスで短い候補トークンのシーンスを予測し、主モデルはその一連のシーン` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.35 | yes | `想的のデコードは小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが短い候補トークンの自動回帰的な順序を予測し、その順序全体が主モデルで一回` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.43 | yes | `想的のデコードは、小さなドラファーモデルと大きな主モデルを組み合わせています：ドラファーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.53 | yes | `展開ステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.57 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.50 | yes | `ステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.97 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokensandmainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Inspeculativedecoding,DrafthammerpredictscandidatetokensandMainmergestheheadword` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.56 | yes | `달은턴스의프라이버시우선면번역도구입니다.이도구는부에연결되지않고,사용자의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.54 | yes | `달팽이는개인정보보호를최우선으로두는면번역도구입니다.이도구는전부사용자의로컬컴퓨터에서실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `달팽이는프라이버시우선시creens번역도구로,모든작업이루투스기기에서실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductionenimageprivilégiéquifonctionneentièrementsurvotrem` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `MoonLensestunoutildetraductiond'écranàprivacitépremièrequifonctionneentièrements` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.42 | yes | `MoonLensestunoutildetraductionparécranquisebasesurlaprotectiondesdonnéespersonne` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.08 | yes | `Leméthodologiededécodagespeculativecomportelacombinaisond'unmodèleminièreetd'unm` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminiaturiséetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.08 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodulairebasiqueetd` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.19 | yes | `Décodagespeculativeassocieunmodéleprincipalplusgrandavecunmodèlededécodeurpluspe` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.16 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandavecunmodèledécodeurpluspeti` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Décodagespeculativesuitunepetitemodélisationdédiéeavecunegrandemodèleprincipale:` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.69 | yes | `Ladécodageparpariestuneméthodequiconsisteàcombinerunmodèleminiaturisédedroneetun` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.85 | yes | `Ladécodageparprojectionestuneméthodequiconsisteàcombinerunmodèleminiaturisédedro` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.28 | yes | `Ladécodageparpiocheestuneméthodecombinantunmodèlederobotiqueminietunmodèleprinci` |
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


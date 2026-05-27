# Translate Quality Eval (routed)

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ✅ PASS (0.05-tolerance)

Quality ≥ Standard ≥ Compact holds for every (fixture, metric).

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Standard | 0.93 | 0.95 | 1.03 | 100% | 0.9 | 63/63 |
| Quality | 0.95 | 0.95 | 1.04 | 100% | 0.5 | 63/63 |
| Compact | 0.74 | 0.57 | 2.14 | 100% | 176.7 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 1.00 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 1.00 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.38 | 1.00 (3/3) | 6.48 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リリースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リリースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.63 | yes | `##リリースノート` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `コマンを以下のように実行します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.55 | yes | `素早い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `速い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルを大きなメインモデルと対応させる推測的なデコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデルが` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルと大きなメインモデルを対応させる推測的なデコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデルが` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.51 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコードが行われます。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メ` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.57 | yes | `デプロイ手は以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.58 | yes | `デプロイ手は以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.60 | yes | `デプロイ手は以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 0.50 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 0.50 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Laméthodededécodagespéculatifimpliquelacombinaisond'unpetitmodèled'ébaucheetd'un` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Laméthodededécodagespéculatifimpliquedecombinerunpetitmodèled'ébaucheetungrandmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Laméthodededécodagespéculatifimpliquelacombinaisond'unpetitmodèled'ébaucheetd'un` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Ledécodagespéculatifestunetechniquequicombinedepetitsmodèlesavecdegrandsmodèles.` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.77 | yes | `Ladécodagespéculatifestuneméthodequicombinedesmodèlesdebrouillondepetitetailleav` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.77 | yes | `Ledécodagespéculatifestuneméthodequicombinedesmodèlesdebrouillondepetitetailleav` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.83 | yes | `SpekulativeDekodierungassoziiertalsModellmiteinemgroßenHauptmodellzurBeschleunig` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.83 | yes | `SpekulativeDekodierungassoziiertalsModellmiteinemgroßenHauptmodellzurBeschleunig` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.83 | yes | `SpekulativeDekodierungassoziiertalsModellmiteinemgroßenHauptmodellzurBeschleunig` |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（コンイルされたバ` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| log-single-line | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (5/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window：label='main'に焦点が失われ、show_mainの前に__MLENS_T_` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:hy-mt2-1.8b-q4_k_m` | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| code-cli-help | code | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.38 | 1.00 (3/3) | 6.48 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 1.00 | yes | `{` |
| structured-json | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 0.76 | 1.00 (3/3) | 1.00 | yes | `{` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リリースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リリースノート` |
| markdown-bullets | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (1/1) | 0.64 | yes | `##リリースノート` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.83 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `コマンを以下のように実行します：` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.65 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。.</p>` |
| html-tags | html | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `<p class="intro">翻字ツールを尊重する翻訳アプリ<strong>MoonLens</strong>へようこそ。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `速い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.55 | yes | `速い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.55 | yes | `速い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測型デコードでは、デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルと大きなメインモデルを対応させる推測型デコードでは、デザイナーデルが候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| long-prose-paragraph | long-prose | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測型デコードでは、デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.59 | yes | `デプロイ手は以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.59 | yes | `デプロイ手は以下の通りです：` |
| indented-multiline | indented | `Direct:hy-mt2-1.8b-q4_k_m` | 0.72 | 1.00 (0/0) | 0.59 | yes | `デプロイ手は以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.48 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.48 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| reverse-ja-to-en | reverse | `Direct:hy-mt2-1.8b-q4_k_m` | 1.00 | 1.00 (0/0) | 2.48 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴퓨터에서완전히실행되는프라이버시우선화면번역도구입니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialitéquis'exécuteentièrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Laméthodededécodagespéculatifimpliquedecombinerunpetitmodèlerédacteuretungrandmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Laméthodededécodagespéculatifimpliquedecombinerunpetitmodèlerédacteuretungrandmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Laméthodededécodagespéculatifimpliquedecombinerunpetitmodèlerédacteuretungrandmo` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Ledécodagespéculatifassocieunpetitrédacteuràunmodèleprincipalplusgrand:lerédacte` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.56 | yes | `Ledécodagespéculatifestunetechniquequicombinedepetitsmodèlesd'ébaucheavecdegrand` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.56 | yes | `Ledécodagespéculatifestunetechniquequicombinedepetitsmodèlesd'ébaucheavecdegrand` |
| pivot-ja-to-fr | multilingual | `Pivot:hy-mt2-1.8b-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.56 | yes | `Ledécodagespéculatifestunetechniquequicombinedepetitsmodèlesd'ébaucheavecdegrand` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `SpekulativeDekodierungwirdmiteinemkleinenEntwurfsmodellundeinemgroßenHauptmodell` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `SpekulativeDekodierungwirdmiteinemkleinenEntwurfsmodellundeinemgroßenHauptmodell` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `SpekulativeDekodierungwirdmiteinemkleinenEntwurfsmodellundeinemgroßenHauptmodell` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.17 | 0.00 (0/16) | 0.14 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.17 | 0.00 (0/16) | 0.13 | yes | `ステラに電話してください。このあと、彼女は3つの赤い袋にこれらのものを詰めて持ってきてくれるでしょう。水曜日の午後には駅で待ち合わせましょう。` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.17 | 0.00 (0/16) | 0.07 | yes | `このパイプラインは「embedded.llm」から2552ms後に翻訳されます。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.83 | yes | `ステラに連絡してください。これらをお店で買ってきてください。新鮮なスナップエンドウが6さじ、厚切りのブルーチーズが5枚、それから弟のボブのおやつも必要です。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.00 (0/1) | 0.20 | yes | ````` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.00 (0/1) | 0.60 | yes | ````` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/1) | 0.44 | yes | `関数`run_translate`は引数を受け取ります。この関数は非同期に実行され、結果としてタスクの成功または失敗を返し、エラーが発生した場合にはProvid` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.25 (1/4) | 11.48 | yes | `このプログラムを使用するには、`moonlens`という名前のプロファイラを起動してください。プロファイラが起動したら、`-verbose`オプションを付けてプ` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.42 | 0.00 (0/4) | 1.86 | yes | ````` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/4) | 0.49 | yes | `使用方法：プロメテウスネーム：プロファイル名：パス：詳細レベル：0,0,1920,1080` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.53 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.53 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.53 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/4) | 0.26 | yes | `ユーザーが日本語の設定ファイルを更新する必要があります。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/4) | 0.75 | yes | `ステラに連絡してください。これらのものを店で購入するように頼んでください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/1) | 0.38 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/1) | 0.41 | yes | `-Addedacheckfor`_`intokenswithspecialmeanings(e.g.,`code_identifier`,`module_pat` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.00 (0/1) | 0.26 | yes | `Pleasecallmeat(123)-456-7890togetthelatesttranslation.` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.00 (0/4) | 0.14 | yes | `<\|im_start\|>user` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.62 | 0.00 (0/4) | 0.90 | yes | ````` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/4) | 0.24 | yes | `ベンチマークレポートをdoc/benchmarks.txtに書きます。` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.73 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.87 | yes | `ステラに連絡してください。これらのアイテムをお店で購入し、持ってきてください。新鮮なスナップエンドウが6皿、厚切りのブルーチーズが5枚、それから弟のボブのおやつ` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.73 | yes | `ステラに連絡してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 4.54 | yes | `こんにちは、世界でHello,world!ThisisatranslationofthetextinJapanese.` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 5.85 | yes | `こんにちは、ステラ。ここに持ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、そしてボブのおやつと少しだけ持ってきましょう。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.55 | yes | `この素早い茶色の狐は、怠け者の犬を飛び越えます。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `この素早い茶色の狐は怠け者の犬を飛び越えます。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.27 | yes | `「素早い茶色の狐が怠け者の犬を飛び越える」という文は、「素早い茶色の狐が怠け者の犬を跳び越える」と翻訳できます。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 0.54 | yes | `このテキストを日本語に翻訳します。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 0.54 | yes | `このテキストを日本語に翻訳しました。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 0.46 | yes | `この文章を日本語に翻訳すると、以下のようになります。` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.56 | 1.00 (0/0) | 0.42 | yes | `Step1:Stoptherunningcontainer` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 0.71 | yes | `Pleaseaskhertobringthefollowingitemswithyoufromthestore:Sixspoonsoffreshsnowpeas` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 0.23 | yes | `Thecontainerisnowrunningwiththesamevolumemounts.` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.73 | yes | `Thedynamicdecoderpredictsthecandidatetokensandverifiesthemwithamaindecodertocomb` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.44 | 1.00 (0/0) | 3.88 | yes | `TheDjangomodelisacandidatetokenthatpredictsthenexttokensinthetrail.Themainmodelv` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.63 | yes | `Inthecontextofatrademark,itisimportanttoensurethatanypotentialsynonymsarealsotra` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Thisprivacy-firstscreentranslationtoolwillrunonyourlocalmachineアゴニアル.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.77 | yes | `Thisprivacy-firstscreentranslationtoolrunsentirelyonyourlocalmachine・・・・` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.76 | yes | `기한의Pencil-free,privacy-focusedtranslationtoolrunsonyourlocalmachine徒然草.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.34 | 1.00 (0/0) | 15.06 | yes | `MoonLensestuntooldetransliteurpourlacommunicationenfrançais,utilisantvotrecasque` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Lelunettessontuntooldetranslationpourlaprivacy,quiestutilisésurvotrecasuleleprog` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.35 | 1.00 (0/0) | 17.70 | yes | `Lemoonlensestuneapplicationpourlatraductionenfrançaisquiaimelefairesurvotremachi` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.35 | 1.00 (0/0) | 15.11 | yes | `Ledecodingestunemethodespeculatifquicombineunemodellededécodererunappareillargem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.43 | yes | `Lesdecodingmethodssontdemanièrespeculatif` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 15.45 | yes | `Lemodèlededecodingspéculatalestunensemblededeuxacteurs:unemainmodelleetuneminimo` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.31 | yes | `Lerapporteurdel'autorateurest-ilcorrect` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `Ledrafterpredicteantdel'authoressentiallyアラバマ州` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `LedrafterpredictelatokensauthoregressivelyGeneraux` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Letraitementdecodeestunetechniqueutiliséepourlesmachinesminiaturesetlargementpou` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.92 | yes | `Leportagedeladécouvrirestunetechniqueutilisantunmodèled'auto-développement.` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.54 | yes | `Letraitementdedécouvrirlecontenuenfrançaisestuneétudeappliquéeparunmodèled'analy` |
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


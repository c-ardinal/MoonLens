# Translate Quality Eval

**Label**: after-prompt-fix-v2-inline-ws

**Samples per fixture**: 1

**Fixtures evaluated**: 15

## Tier ranking (sorted by id_score)

| Tier | ws | id | len_ratio | translated | decode tok/s |
|---|---:|---:|---:|---:|---:|
| Compact (Qwen 2.5 0.5B) | 0.91 | 0.93 | 0.83 | 100% | 43.1 |
| Quality- (Qwen 3 4B Instruct 2507) | 0.93 | 0.93 | 0.83 | 100% | 6.9 |
| Quality-- (Qwen 3.5 4B) | 0.93 | 0.93 | 0.83 | 100% | 6.9 |
| Quality (Qwen 2.5 7B) | 0.86 | 0.93 | 0.94 | 100% | 5.7 |
| Compact+ (Qwen 3 0.6B) | 0.91 | 0.87 | 0.76 | 100% | 36.0 |
| Standard+ (Qwen 3 1.7B) | 0.93 | 0.87 | 0.83 | 100% | 18.2 |
| Edge (Gemma 4 E2B) | 0.91 | 0.87 | 0.77 | 100% | 12.5 |
| Edge+ (Gemma 4 E4B) | 0.92 | 0.87 | 0.85 | 100% | 7.1 |
| Compact++ (Qwen 3.5 0.8B) | 0.86 | 0.80 | 0.82 | 100% | 31.0 |
| Standard++ (Qwen 3.5 2B) | 0.91 | 0.78 | 0.79 | 100% | 16.0 |
| Standard (Qwen 2.5 1.5B) | 0.90 | 0.74 | 0.74 | 100% | 21.2 |
| JA-Standard+ (llm-jp 3.1 1.8B) | 0.73 | 0.62 | 1.08 | 100% | 17.2 |
| JA-Compact (llm-jp 3 980M) | 0.67 | 0.43 | 0.71 | 93% | 28.7 |

## Per-tier × fixture detail

### Compact (Qwen 2.5 0.5B) (qwen2.5-0.5b-instruct-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.39 | 1.00 (16/16) | 1.07 | yes | `2026-05-18T00:51:11.674022ZINFO:embedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| structured-json | structured | 0.72 | 1.00 (3/3) | 0.94 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.92 | yes | `[provider_configs.embedded.llm]` |
| markdown-bullets | markdown | 1.00 | 1.00 (1/1) | 0.55 | yes | `##ソード通知` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.90 | yes | `Benchmarksを実行するには：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.93 | yes | `<blockquoteclass="intro">お会いするための<strong>MoonLens</strong>は、格式を尊重しているTranslation` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `速いい熊、そして優しいの犬に跳ね返って。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアと実行配置によって変動します。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `スペクトラルのdecodingは、小さなドラッタモデルと大きな主模様を組み合わせる。ドラッタモデルが最も重要なトークンの一部に取り込むため、一時的に回する順序で` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.83 | yes | `Inaninvestmentdecoding,theDFLtokenispredictedbythemodelandthencombinedwithalisto` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.66 | yes | `デプローーションステップsは：` |

### Quality- (Qwen 3 4B Instruct 2507) (qwen3-4b-instruct-2507-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.84 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:ンベデッドLLMのウォームアップ：38ms（コンパイルみバック` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.84 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'にフォースが失われたshow_mainートが実行される前に` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.88 | yes | `使い方:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | 0.76 | 1.00 (3/3) | 0.82 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 1.00 (1/1) | 0.63 | yes | `##リースノート ` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.88 | yes | `次のコマンドでベンチマークを実行します：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.65 | yes | `<pclass="intro"><strong>MoonLens</strong>へようこそ。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `速いブラウンの狐が、怠惰な犬の上を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `パフォーンスはハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `推測的解码は、小型のドレッターモデルと大きなメインモデルを組み合わせます。ドレッターモデルは自動回帰的に短い候補トークンのシーンスを予測し、その後メインモデルが` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.27 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.63 | yes | `デプロイ手順は以下の通りです：` |

### Quality-- (Qwen 3.5 4B) (qwen3.5-4b-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.85 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'のフォーカスがshow_mainルートの実行前に失われました` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.90 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,10` |
| structured-json | structured | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 1.00 (1/1) | 0.69 | yes | `##リリースノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.88 | yes | `以下のコマンドでベンチマークを実行します:` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.64 | yes | `<pclass="intro">フォーマットを尊重する翻訳ツール<strong>MoonLens</strong>にようこそ。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `素早い茶色のキツネは、怠け者の犬を飛び越えた。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `パフォーマンスはハードウェアとランタイム設定によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `推測デコードは、小さなドラフトモデルと大きなメインモデルをペアリングします：ドラフトモデルが自己回帰的に短い候補トークンのシーケンスを予測し、その後メインモデル` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelverifi` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.59 | yes | `デプロイ手順は以下の通りです：` |

### Quality (Qwen 2.5 7B) (qwen2.5-7b-instruct-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 0.44 | 1.00 (5/5) | 1.81 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | 0.76 | 1.00 (3/3) | 0.88 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.69 | yes | `##リースノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.82 | yes | `ベンチマークを実行するには：` |
| html-tags | html | 1.00 | 1.00 (1/1) | 0.91 | yes | `<pclass="intro">Welcometo<strong>MoonLens</strong>,thetranslationtoolthatrespect` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `速い茶色の狐は怠け者な犬を飛び越えます。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.89 | yes | `PERFORMANCEVARYSACROSSHARDWAREANDRUNTIMECONFIGURATIONS.` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.63 | yes | `推測的なデコーィングは、小さなドラッフモデルと大きなメインモデルを組み合わせます：ドラッフモデルは自動regressively）で短い候補トークンのシーンスを予` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelbatch-` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.58 | yes | `デプロイ手順は以下の通りです：` |

### Compact+ (Qwen 3 0.6B) (qwen3-0.6b-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.81 | yes | `moonlens--profile[NAME]--config[PATH]--verbosecapture--region0,0,1920,1080` |
| structured-json | structured | 0.67 | 1.00 (3/3) | 0.66 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.93 | yes | `providers.configs."embedded.llm"` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.72 | yes | `##ビューノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.94 | yes | `バーンを運ぶように、以下のコマンドを実行します：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.41 | yes | `<pclass="intro">月のレンズ、翻译ツールでフォーットを尊重する。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `速いブルーのオーストゥンは、ヨールドに渡ります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `ハードウェアと実行環境の変化によって、パフォーンスが異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.38 | yes | `予測可能なドレッターモデルと主モデルのペアとして、ドレッターは短いカタログの候補トークンを自動的に重複して予測し、主モデルは全体のカタログを純な進化パスで確認し` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.53 | yes | `IntheDCFmodel,thecandidatetokensarepredictedandthemainmodelconfirmsagreementinac` |
| indented-multiline | indented | 0.54 | 1.00 (0/0) | 0.47 | yes | `部署のステップは以下の通りです：` |

### Standard+ (Qwen 3 1.7B) (qwen3-1.7b-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.87 | yes | `使用:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | 0.76 | 1.00 (3/3) | 0.82 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.52 | yes | `##リースノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.79 | yes | `ベンチを実行します：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.48 | yes | `<pclass="intro">モーングレンズへようこそ、フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `パフォーンスはハードウェアおよびランタイムの構成によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的な解は、小さなドレッタモデルと大きなメインモデルを組み合わせます。ドレッタモデルは短い候補トークンのシーンスを自己ルーィングして予測します。その後、メインモ` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.23 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelaggreg` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.56 | yes | `デプロイステップは以下の通りです:` |

### Edge (Gemma 4 E2B) (gemma-4-e2b-it-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.93 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 1.00 (1/1) | 0.68 | yes | `##リリースノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.85 | yes | `以下のベンチを実行してください：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.67 | yes | `<pclass="intro">ようこそ<strong>MoonLens</strong>へ、フォーマットを尊重する翻訳ツールです。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `すばやい茶色のキツネが怠惰な犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーマンスはハードウェアと実行時の構成によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `Speculativedecodingは、小さなドラフターモデルとより大きなメインモデルを組み合わせます。ドラフターは候補トークンの短いシーケンスを自己回帰的に` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.88 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelverifi` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.58 | yes | `デプロイ手順は次のとおりです。` |

### Edge+ (Gemma 4 E4B) (gemma-4-e4b-it-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.85 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'のフォーカスが、show_mainルート実行前に失われました` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.93 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.88 | yes | `使用法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | 0.76 | 1.00 (3/3) | 0.91 | yes | `{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.62 | yes | `##リリースノート` |
| markdown-codeblock | markdown | 0.83 | 1.00 (4/4) | 1.06 | yes | `以下のテキストを日本語に翻訳してください。` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.65 | yes | `<pclass="intro">フォーマットを尊重する翻訳ツール、<strong>MoonLens</strong>へようこそ。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `すばやい茶色のキツネは、怠惰な犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.53 | yes | `パフォーマンスはハードウェアやランタイムの設定によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `推測的デコーディングは、小さなドラフターモデルとより大きなメインモデルをペアにします。ドラフターが自己回帰的に短い候補トークンのシーケンスを予測し、その後メイン` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.15 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelvalida` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.62 | yes | `デプロイ手順は以下の通りです。` |

### Compact++ (Qwen 3.5 0.8B) (qwen3.5-0.8b-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.37 | 1.00 (1/1) | 1.06 | yes | `pubasyncfnrun_translate(` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.80 | yes | `設定：moonlens--profileNAME--configPATH--verbosecapture--region0,0,1920,1080` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 0.85 | 0.00 (0/1) | 0.71 | yes | `##リリースノート` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.92 | yes | `ベンチを動かすために以下のコマンドを実行します：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.60 | yes | `<pclass="intro">欢迎来到<strong>MoonLens</strong>，这是一款尊重格式翻译工具。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.68 | yes | `quickbrownfoxjumpsoverlazydog.` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーマンスはハードウェアとリソース配置によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `推測解碼は、小型のデプローターモデルと大きなメインモデルをペアリングします。デプローターが短いシーンの候補トークンに対して自回帰で予測し、メインモデルは全体の流` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.47 | yes | `Inprobabilisticdecoding,thedraftmodelpredictscandidatetokenswhilethemainmodelsum` |
| indented-multiline | indented | 0.60 | 1.00 (0/0) | 0.45 | yes | `部署手順は以下の通りです：` |

### Standard++ (Qwen 3.5 2B) (qwen3.5-2b-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | 1.00 | 1.00 (5/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:focuslostonlabel='main'beforeshow_mainrout` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 0.75 (3/4) | 0.74 | yes | `使用例：moonlens[--profile名前][--configパス][--verbose]撮影--領域0,0,1920,1080` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.54 | yes | `##更新情報` |
| markdown-codeblock | markdown | 1.00 | 1.00 (4/4) | 0.79 | yes | `ベンチを動かします：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.50 | yes | `<pclass="intro">MoonLensにようこそ。フォーマットを尊重する翻訳ツールです。</p>` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `急げいぼんねんこうはあずきぬをよこします。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `パフォーマンスはハードウェアと実行環境の構成によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.46 | yes | `推測的デコードは、小さなドラフトモデルと大きなメインモデルを組み合わせます。ドラフトモデルは自己回帰的に短く候補のトークン列を予測し、メインモデルがそれを1回の` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.80 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelvalida` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.50 | yes | `デプロイステップは以下の通りです：` |

### Standard (Qwen 2.5 1.5B) (qwen2.5-1.5b-instruct-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.69 (11/16) | 0.86 | yes | `2026年5月18日00:51:11.674022ZINFOembedded_llm_perf:38イクロソフトLLMの初期化：コンパイルされたバックエンドのキ` |
| log-single-line | logs | 1.00 | 0.40 (2/5) | 0.71 | yes | `2026年5月18日00:51:11ーイトtauri::window:mainラベルのフォースが前のshow_mainルートで失われた` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.93 | yes | `asyncfnrun_translate(self,args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 1.00 (4/4) | 0.99 | yes | `月光線:`--profileNAME`,`--configPATH`,`--verbose`を指定して、capture--region0,0,1920,1080` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[provider.configs."embedded.llm"]` |
| markdown-bullets | markdown | 1.00 | 0.00 (0/1) | 0.66 | yes | `##リースノート` |
| markdown-codeblock | markdown | 0.83 | 1.00 (4/4) | 0.87 | yes | `以下に日本語で翻します：` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.48 | yes | `お世話になっております<strong>月レンズ</strong>。フォーットを尊重する翻ツールです。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.68 | yes | `quickbrownfoxjumpsoverlazydog.` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.40 | yes | `パフォーンスはハードウェアや実行構成に依存します。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.38 | yes | `想的のデコードは、小さなプレーーと大きな主モデルを組み合わせています：プレーーが自動回帰的に短い候補トークンのシーンスを予測し、その後主モデルはそのシーンス全体` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,DRAFTmodelspredictcandidatetokensandmainmodelsaggregatethe` |
| indented-multiline | indented | 0.72 | 1.00 (0/0) | 0.49 | yes | `Deployステップ：` |

### JA-Standard+ (llm-jp 3.1 1.8B) (llm-jp-3.1-1.8b-instruct4-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.27 | 0.69 (11/16) | 0.96 | yes | `以下のテキストを日本語に翻訳してください。` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前1時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.95 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | 1.00 | 0.00 (0/4) | 1.42 | yes | `ムーンレンズの[-profileNAME]、[-configPATH]、そして[-verbose]オプションを使ってビデオをキャプチャします。そのために必要な設` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 0.67 | 0.00 (0/4) | 0.47 | yes | ``embedded_llm_perf.py`,`moonlens_lib::pipeline.py`` |
| markdown-bullets | markdown | 0.50 | 0.00 (0/1) | 5.28 | yes | `申し訳ありませんが、提供されたテキストの翻訳は含まれていません。このタスクを完了するためには、まずそのテキスト全体を入力し、次に「ja」と指定する必要があります` |
| markdown-codeblock | markdown | 0.81 | 1.00 (4/4) | 0.85 | yes | `次のコードを実行してください。` |
| html-tags | html | 0.50 | 0.00 (0/1) | 0.45 | yes | `<strong>MoonLens</strong>は、フォーマットを尊重する翻訳ツールです。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `素早い茶色の狐が怠け者の犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.45 | yes | `次のテキストを日本語に訳してください。` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.22 | yes | `Inaprobabilisticdecodingprocess,thedrafftmodelpredictscandidatetokensandthemainm` |
| indented-multiline | indented | 0.56 | 1.00 (0/0) | 0.63 | yes | `以下の文章を日本語に訳します。` |

### JA-Compact (llm-jp 3 980M) (llm-jp-3-980m-instruct3-q4_k_m)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.17 | 0.00 (0/16) | 0.18 | yes | `ステラに電話してください。これらのものをお店で買ってきてもらいましょう。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも持` |
| log-single-line | logs | 0.50 | 0.40 (2/5) | 1.02 | yes | `このテキストを翻訳すると、次のようになります：` |
| code-rust-signature | code | 0.50 | 0.00 (0/1) | 0.42 | yes | `タスクを実行するには、`run_translate`関数を呼び出してください。`TranslateArgs`は翻訳の対象となるテキストとソース言語、ターゲット言` |
| code-cli-help | code | 1.00 | 0.00 (0/4) | 0.89 | yes | `「このカメラはプロファイル名で設定され、パスをキャプチャしてverboseなフラグを有効にし、デフォルトの地域で0,0,1920,1080ピクセルで撮影します。` |
| structured-json | structured | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | 0.50 | 0.00 (0/4) | 1.55 | yes | `"Qwen"という名前のライブラリをインストールするには、`embedded.llm`というファイルを使用します。このファイルはQwenという名前のモジュールで` |
| markdown-bullets | markdown | 0.50 | 0.00 (0/1) | 0.22 | yes | `Pleaseletmeknowifyouneedanyfurtherassistance.` |
| markdown-codeblock | markdown | 0.62 | 0.00 (0/4) | 0.40 | yes | ````` |
| html-tags | html | 1.00 | 0.00 (0/1) | 0.00 | **no** | `` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 1.27 | yes | `Thequickbrownfoxjumpsoverthelazydogいっしょに遊んでくれる人をさがしています。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.60 | yes | `システムのパフォーマンスはハードウェアと実行設定によって大きく変動します。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.37 | yes | `小さなライターモデルと大きなメインモデルの間に翻訳を行います。ライターは候補者の短いシーケンスを迅速に選び、全体的なシーケンスが一つのフォワードパスで正確に一致` |
| reverse-ja-to-en | reverse | 0.44 | 1.00 (0/0) | 2.38 | yes | `Thewaytotranslatethistextisasfollows:` |
| indented-multiline | indented | 0.44 | 1.00 (0/0) | 0.37 | yes | `コンテナを停止してインフリアルリクエストを待機させます。ワークキューから新しいイメージを取り出します。新しいコンテナに同じボリュームマウントを設定します。` |

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
| reverse-ja-to-en | reverse | ja → en | 60 |
| indented-multiline | indented | en → ja | 209 |


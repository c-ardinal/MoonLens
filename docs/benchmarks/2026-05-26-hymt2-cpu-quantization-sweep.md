# Specialist Translation Model Bench

**Samples per fixture**: 2

**Fixtures evaluated**: 12

## Ranking (sorted by id_score)

| Model | ws | id | translated | decode tok/s |
|---|---:|---:|---:|---:|
| Hy-MT2 1.8B @ temp=0.3 | 0.96 | 0.98 | 100% | 19.0 |
| Hy-MT2 1.8B @ temp=0.2 | 0.95 | 0.98 | 100% | 18.8 |
| Hy-MT2 1.8B @ temp=0.7 (publisher recommended) | 0.92 | 0.98 | 100% | 19.1 |
| llm-jp-3.1 1.8B (Standard baseline) | 0.83 | 0.83 | 100% | 17.5 |
| llm-jp-3 980M (Compact baseline) | 0.77 | 0.68 | 100% | 30.2 |

## Per-fixture detail

### Hy-MT2 1.8B @ temp=0.3

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.87 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップが完了しました。38ミリ秒（バッ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 1.00 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示前に失われたtauri::window:show` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.86 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.89 | yes | `バンケットを以下のコマンドで実行します：` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `速い茶色のキツネが、怠け者の犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける定的デコードです。デザイナーは候補トークンの短いシーケンスを自動的に進展的に予測し、その後メインモデルが` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルを大きなメインモデルと対応付ける推測的なデコードが行われます。デザイナーは候補トークンの短いシーケンスを自動的に進行して予測し、その後メイン` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.48 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelcon` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.25 | yes | `MoonLensestuneoutildetraductiond'écrancentrésurlaprotectiondesdonnées,quifonctio` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |

### Hy-MT2 1.8B @ temp=0.2

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 1.00 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示前に失われたtauri::window:show` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.72 | yes | ````bash` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.87 | yes | `バンクを以下のコマンドで実行します：` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `速い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `速い茶色のキツネが、怠け者の犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルと大きなメインモデルを対応付ける推測的デコードでは、デザイナーデルが候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `小さなデザイナーデルと大きなメインモデルを対応付ける推測型デコードでは、デザイナーが候補トークンの短いシーケンスを自動的に予測し、メインモデルは一つの前方処理で` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialité,quifonctionneentièr` |

### Hy-MT2 1.8B @ temp=0.7 (publisher recommended)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップが終了しました。38ミリ秒（バッ` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.99 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.86 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」の上でフォーカスが失われた。show_mainルートが実行された前` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.43 | 1.00 (3/3) | 0.67 | yes | ``cargorun--examplebench_specialist_translation--release`を実行して、レポートがdocs/benchmar` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.76 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `素早い茶色の狐が、怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `速い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `ハードウェアと実行環境の設定によってパフォーマンスが異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルを大きなメインモデルと対応させる概念的な符号解読です。デザイナーデルは候補トークンの短いシーケンスを自動的に次予測し、その後メインモデルが一` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける想復読が行われます。デザイナーは候補トークンの短いシーケンスを自動的に進展的に予測し、その後メインモデル` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.47 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelconduc` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.47 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelcon` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.19 | yes | `MoonLensestunoutildetraductiond'écranpriorisantlaconfidentialité,quifonctionneen` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialité,quifonctionneentièr` |

### llm-jp-3.1 1.8B (Standard baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.39 | 0.38 (6/16) | 0.85 | yes | `以下の日本語訳を提供します。` |
| log-rust-tracing-multi | logs | 0.39 | 0.44 (7/16) | 0.85 | yes | `以下の日本語訳を提供します。` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 0.50 | 0.00 (0/4) | 0.67 | yes | ``embedded_llm.configs[0].providers.configs."embedded.llm"`の値を次のように翻訳します：` |
| structured-toml | structured | 0.50 | 0.00 (0/4) | 0.31 | yes | ``embedded_llm_perf`の設定を次のように翻訳します。` |
| markdown-codeblock | markdown | 0.90 | 1.00 (3/3) | 0.92 | yes | `ベンチを走らせるには：` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.67 | yes | `ベンチを走らせるには、次のコマンドを実行します。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが怠け者の犬の上を飛び越えます。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `素早い茶色の狐が怠け者の犬の上を飛び越えます。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.43 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて使用します。このドラフトは候補となるトークンの短いシーケンスを予測し、その後、メインモデルがそのシーケンス` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて使用します。この小さなドラフトは候補となるトークンの短いシーケンスを予測し、長い一致する接頭辞が1回のフォ` |
| reverse-ja-to-en | reverse | 0.37 | 1.00 (0/0) | 4.03 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandamainmodelverifi` |
| reverse-ja-to-en | reverse | 0.44 | 1.00 (0/0) | 2.93 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandamainmodelverifi` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.48 | yes | `MoonLens는사용자의로컬컴터에서완전히실행되는프라이버시우선화면번역도구입니다.城町` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.62 | yes | `MoonLens는사용자의로컬컴터에서완전히실행되는strangereuteicksscreen翻译工具입니다.城町` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.88 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.88 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |

### llm-jp-3 980M (Compact baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.17 | 0.00 (0/16) | 0.14 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-rust-tracing-multi | logs | 0.17 | 0.00 (0/16) | 0.14 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-single-line | logs | 1.00 | 0.00 (0/5) | 0.18 | yes | `このテキストを日本語に翻訳します。` |
| log-single-line | logs | 1.00 | 0.00 (0/5) | 0.35 | yes | `このラベルの「メイン」はプログラムのエントリーポイントを指します。` |
| code-rust-signature | code | 0.38 | 0.00 (0/1) | 1.74 | yes | ````` |
| code-rust-signature | code | 0.39 | 1.00 (1/1) | 1.93 | yes | ````` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 0.50 | 0.25 (1/4) | 0.54 | yes | `"embedded.llm"を"Qwen/Qwen2.5-1.5B-Instruct-GGUF"に翻訳してください。` |
| markdown-codeblock | markdown | 0.62 | 0.00 (0/3) | 0.30 | yes | ````` |
| markdown-codeblock | markdown | 0.43 | 0.00 (0/3) | 0.33 | yes | `このレポートはdoc/benchmarks.txtに書かれている通りに翻訳してください。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 7.00 | yes | `こんにちは、ステラ。彼女にこれらを店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも持ってきてくれる` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `この素早い茶色の狐は、怠け者の犬を飛び越えます。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `この素早い茶色の狐は怠け者の犬を飛び越えます。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.44 | yes | `このテキストを日本語に翻訳します。` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.57 | yes | `このテキストを日本語に翻訳すると、以下のようになります。` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.42 | yes | `Thetrademarktokensarepredictedbythepredictormodel,andthemainmodelverifiesthemtom` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.30 | yes | `Thedynamicdecoderpredictsthetoken,andthemaindecoderverifiesitwithasinglecheck.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.61 | yes | `이Prestigescreentranslationtool는사용자의LaptopCompact에의해실행됩니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.51 | yes | `TotranslatethetextintoKorean,youcanusethistool:` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 18.04 | yes | `LeMoonLensestuntooldetraductionenfrançaisversunemachinelocaleアラバマ州.アラバマ州.アラバマ州.ア` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.77 | yes | `LeMoonLensestuntooldetraductionenfrançaisversunemachinelocaleアラバマ州.アラバマ州` |


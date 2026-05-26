# Specialist Translation Model Bench

**Samples per fixture**: 2

**Fixtures evaluated**: 12

## Ranking (sorted by id_score)

| Model | ws | id | translated | decode tok/s |
|---|---:|---:|---:|---:|
| Hy-MT2 1.8B @ temp=0.2 | 0.96 | 0.98 | 100% | 18.8 |
| Hy-MT2 1.8B @ temp=0.7 (publisher recommended) | 0.93 | 0.98 | 100% | 19.3 |
| Hy-MT2 1.8B @ temp=0.3 | 0.95 | 0.97 | 100% | 19.4 |
| llm-jp-3.1 1.8B (Standard baseline) | 0.83 | 0.90 | 100% | 17.6 |
| llm-jp-3 980M (Compact baseline) | 0.78 | 0.71 | 100% | 31.0 |

## Per-fixture detail

### Hy-MT2 1.8B @ temp=0.2

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（バックエンドカー` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.84 | yes | `コマンを以下のように実行します：` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.89 | yes | `バッチを実行するには、次のようにします：` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `速い茶色のキツネが、怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `速い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルと大きなメインモデルを対応させる推測型デコードでは、デザイナーデルが候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルを大きなメインモデルと対応付ける推測的なデコードが行われます。デザイナーは候補トークンの短いシーケンスを自動的に前後して予測し、その後メイン` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelconduc` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.48 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondesdonnées,quifonctionnee` |

### Hy-MT2 1.8B @ temp=0.7 (publisher recommended)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.87 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンディッドLLMのウームアップが完了：38ミリ秒（バックエン` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.96 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_mainが表示される前に失われたtaur` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.84 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたruouteran` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.95 | yes | `[providers.configs["embedded.llm"]]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.95 | yes | `[providers.configs["embedded.llm"]]` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.72 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.71 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `速い茶色のキツネが、怠けた犬の上を跳びる。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.46 | yes | `小さな作成モデルと大きなメインモデルをペアにして推論する：作成モデルは候補トークンの短いシーケンスを自動的に延学習し、その後メインモデルが一回のフォワードパスで` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルをより大きなメインモデルと対応させる推測的なデコードでは、デザイナーデルが候補トークンの短い系列を自動的に予測し、その後メインモデルが一つの` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.40 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelverifi` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelconduc` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialité,quifonctionneentièr` |

### Hy-MT2 1.8B @ temp=0.3

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.87 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップが完了しました。38ミリ秒（バ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.89 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 0.75 (3/4) | 0.86 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 0.92 | yes | `バッチを実行するには、次のコマンドを使用します：` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.71 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `速い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `速い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の構成によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける定的デコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデルが` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルを大きなメインモデルと対応させる推測型デコードでは、デザイナーデルが候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.17 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelconduc` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.18 | yes | `MoonLensestuneoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneent` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.20 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondesdonnées,quifonctionnee` |

### llm-jp-3.1 1.8B (Standard baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.88 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 0.67 | 0.50 (2/4) | 0.92 | yes | ``embedded_llm_perf`の設定を次のように翻訳します。` |
| structured-toml | structured | 0.61 | 0.50 (2/4) | 1.27 | yes | `以下のテキストを日本語に翻訳します。` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.68 | yes | `ベンチを走らせるには、以下のコマンドを実行します。` |
| markdown-codeblock | markdown | 0.71 | 0.33 (1/3) | 0.76 | yes | `ベンチで実行するには、次のコマンドを使用します：` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `素早い茶色のキツネが怠け者の犬を飛び越えます。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.43 | yes | `素早い茶色の狐が怠け者の犬を飛び越える` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて、特定の予測タスクに使用します。drafterは候補となるトークン列を繰り返し予測し、その後mainmod` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.53 | yes | `以下の文章を日本語に翻訳します。` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.18 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| reverse-ja-to-en | reverse | 0.40 | 1.00 (0/0) | 3.92 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandamainmodelverifi` |
| multi-en-to-ko | multilingual | 0.39 | 1.00 (0/0) | 2.00 | yes | `ムーンレンズ는사용자의로컬컴터에서완전히실행되는프라이버시우선화면번역도구입니다.城町` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.52 | yes | `MoonLens는사용자의로컬컴터에서완전히실행되는strangerscreen翻译工具입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.88 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.88 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |

### llm-jp-3 980M (Compact baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.17 | 0.00 (0/16) | 0.14 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-rust-tracing-multi | logs | 0.17 | 0.00 (0/16) | 0.14 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| log-single-line | logs | 1.00 | 0.00 (0/5) | 0.36 | yes | `このウィンドウがラベル「メイン」の前に表示されるようにしてください。` |
| log-single-line | logs | 1.00 | 0.00 (0/5) | 0.18 | yes | `このテキストを日本語に翻訳します。` |
| code-rust-signature | code | 0.50 | 0.00 (0/1) | 0.50 | yes | `関数を呼び出してください。`run_translate`という名前の関数を引数として渡してください。`text`と`source_language`、および`t` |
| code-rust-signature | code | 0.36 | 1.00 (1/1) | 3.65 | yes | ````` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.90 | 0.00 (0/3) | 1.27 | yes | ````` |
| markdown-codeblock | markdown | 0.43 | 0.00 (0/3) | 0.27 | yes | `このレポートはdoc/ベンチマーク/ベンチマークス/を翻訳してください。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.85 | yes | `こんにちは、世界です。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `この素早い茶色の狐は、怠け者の犬を飛び越えます。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `この素早い茶色の狐は怠け者の犬を飛び越えます。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 1.24 | yes | `ステラに電話してください。これらのものをお店で買ってきてください。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 1.23 | yes | `ステラに電話して、これらのものをお店で買ってきてもらいましょう。新鮮なスナップエンドウを6さじ、厚切りのブルーチーズを5枚、それから弟のボブのおやつも。` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.57 | yes | `このテキストを日本語に翻訳すると、次のようになります。` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 0.38 | yes | `このテキストを日本語に翻訳します。` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.10 | yes | `Thecodewillpredictthecandidatetokensandvalidatethemwithamainmodel.` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.17 | yes | `Thedynamicdecodingmethodpredictsthecandidatetokensandchecksthemwithamainmodeltoc` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.51 | yes | `TotranslatethetextintoKorean,youcanusethistool:` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.76 | yes | `Thisprivacy-firstscreentranslationtoolrunssolelyonyourlocalmachineiele.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.83 | yes | `LeMoonLensestunetooldetranslationverslefrançais,utilisantvotrelocalmachine徒然草` |
| multi-en-to-fr | multilingual | 0.40 | 1.00 (0/0) | 7.87 | yes | `LeMoonLensestunenjeudel'installationpourlatranslationdestextes,quiutiliseunemach` |


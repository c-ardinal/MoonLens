# Specialist Translation Model Bench

**Samples per fixture**: 3

**Fixtures evaluated**: 12

## Ranking (sorted by id_score)

| Model | ws | id | translated | decode tok/s |
|---|---:|---:|---:|---:|
| Hy-MT2 1.8B @ temp=0.3 | 0.93 | 0.98 | 100% | 18.7 |
| Hy-MT2 1.8B @ temp=0.7 (publisher recommended) | 0.92 | 0.98 | 100% | 18.8 |
| Hy-MT2 1.8B @ temp=0.2 | 0.93 | 0.98 | 100% | 18.7 |
| llm-jp-3.1 1.8B (baseline) | 0.88 | 0.89 | 100% | 17.1 |

## Per-fixture detail

### Hy-MT2 1.8B @ temp=0.3

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.88 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンデッドLLMのワームアップが完了しました。38ミリ秒で（コ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップ時間：38ミリ秒（バックエンドカ` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 1.00 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示前に失われたtauri::window:show` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.96 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_mainが表示される前に失われたtaur` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.75 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.76 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.75 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `速い茶色のキツネが、怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `素早い茶色のキツネが、怠けた犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザインモデルと大きなメインデルを対応させる説的なデコードです。デザインモデルは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインデルが一` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザインモデルと大きなメインデルを対応させる推測的なデコードです。デザインモデルは候補トークンの短いシーケンスを自動的に進展的に予測し、その後メインデルが` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.51 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデ` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.53 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.35 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.17 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneenti` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |

### Hy-MT2 1.8B @ temp=0.7 (publisher recommended)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.87 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンインドされたLLMのウームアップ：38ミリ秒（コンイルされ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップ時間：38ミリ秒（バックエンドカ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップ時間：38ミリ秒（バックエンドカ` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 1.09 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示される前に、フォーカスが失われたtauri::wind` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 1.13 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示される前に、window上で失われたtauri:` |
| code-rust-signature | code | 0.37 | 1.00 (1/1) | 4.48 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.77 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.76 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.77 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.77 | yes | `こんにたる世界です。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `素早い茶色の狐が、怠け者の犬の上に跳びる。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.43 | yes | `速い茶色の狐が、怠けた犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.52 | yes | `速い茶色のキツネが、怠惰な犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルと大きなメインモデルを対応させる定的解読方式です。デザイナーは候補語の短い系列を自動的に進行式に予測し、その後メインモデルが一度の正向きのパ` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `小さなデザイナーデルを大きなメインモデルと対応させる推測的デコードです。デザイナーは候補トークンからなる短い序列を自動的に次予測し、その後メインモデルが一回の前` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルとより大きなメインモデルを対応付ける定的解码方式です。デザイナーは候補トークンの短いシーケンスを自動的に進行して予測します。その後、メインモ` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.38 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.42 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodeldoe` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.43 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelperfor` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.15 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaconfidentialité,quifonctionneentièr` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.19 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneenti` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.20 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondesdonnées,quifonctionnee` |

### Hy-MT2 1.8B @ temp=0.2

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップ：38ミリ秒（バックエンドカール` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.85 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップ時間：38ミリ秒（バックエンドカ` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップが完了しました。38ミリ秒（バッ` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 1.00 | 0.80 (4/5) | 0.97 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.75 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.75 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.77 | yes | ````bash` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `素早い茶色の狐が、怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の構成によって性能が異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.44 | yes | `ハードウェアと実行環境の設定によって性能が異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.49 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける定的デコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデルが` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.50 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的デコードです。デザイナーは候補トークンの短いシーケンスを自動的に前進的に予測し、その後メインモデル` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコードです。デザイナーは候補トークンの短いシーケンスを自動的に次予測し、その後メインモデルが一` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.35 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelverifi` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.45 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.35 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.44 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.19 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneenti` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |

### llm-jp-3.1 1.8B (baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-rust-tracing-multi | logs | 0.39 | 0.44 (7/16) | 0.87 | yes | `以下のテキストを日本語に翻訳します。` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.88 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.88 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| code-rust-signature | code | 0.56 | 1.00 (1/1) | 1.02 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 0.61 | 0.75 (3/4) | 1.21 | yes | `以下のテキストを日本語に翻訳します。` |
| structured-toml | structured | 0.85 | 0.50 (2/4) | 1.03 | yes | `以下のテキストを日本語に翻訳します。` |
| structured-toml | structured | 0.50 | 0.25 (1/4) | 0.60 | yes | `Qwen2.5-1.5B-Instruct-GGUFリポジトリから0.4の温度で8192コンテキストを使用して学習するモデルです。` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.67 | yes | `ベンチを走らせるには、次のコマンドを実行します。` |
| markdown-codeblock | markdown | 0.71 | 0.33 (1/3) | 0.67 | yes | `ベンチを走らせるためには以下のコマンドを実行します。` |
| markdown-codeblock | markdown | 0.83 | 1.00 (3/3) | 1.36 | yes | `以下のテキストを日本語に翻訳します。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `素早い茶色の狐が怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `素早い茶色の狐が怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `素早い茶色の狐が怠け者の犬を飛び越える。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.39 | yes | `性能はハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.37 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて使用します。この小さなドラフトは候補となるトークンの短いシーケンスを予測し、長い一致する接頭辞があるかどう` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.31 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせます。この小さなドラフトは候補となるトークンの短いシーケンスを予測し、長い一致する接頭辞が見つかったらそれを` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.55 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて、次のような処理をします。まず、小さなドラフトモデルは候補となるトークンの短い列を予測します。その後、その` |
| reverse-ja-to-en | reverse | 0.44 | 1.00 (0/0) | 3.30 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.62 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.98 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.13 | yes | `ムーンレンズ는로컬{})` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.13 | yes | `ムーンレンズ는로컬{})` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 1.31 | yes | `ムーンレンズ는struggeｌｌｙトラウト한사용자auteonからだromee를드하고entirelyonyourlocalmachine에서completly` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.05 | yes | `MoonLensestunoutildetraductionprivéequifonctionneentièrementsurvotremachinelocal` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.92 | yes | `MoonLensestunoutildetraductionprivéequifonctionneentièrementsurvotremachinelocal` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.88 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |


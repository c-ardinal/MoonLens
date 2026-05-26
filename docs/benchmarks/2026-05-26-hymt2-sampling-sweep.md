# Specialist Translation Model Bench

**Samples per fixture**: 3

**Fixtures evaluated**: 12

## Ranking (sorted by id_score)

| Model | ws | id | translated | decode tok/s |
|---|---:|---:|---:|---:|
| Hy-MT2 1.8B @ temp=0.7 (publisher recommended) | 0.61 | 0.98 | 100% | 18.2 |
| Hy-MT2 1.8B @ temp=0.2 | 0.59 | 0.98 | 100% | 18.6 |
| Hy-MT2 1.8B @ temp=0.3 | 0.56 | 0.98 | 100% | 18.3 |
| llm-jp-3.1 1.8B (baseline) | 0.85 | 0.93 | 100% | 17.2 |

## Per-fixture detail

### Hy-MT2 1.8B @ temp=0.7 (publisher recommended)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.87 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（バックエンドカー` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.87 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンッドLLMのウームアップが完了しました。38ミリ秒で（バッ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.86 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップが完了しました：38ミリ秒（バッ` |
| log-single-line | logs | 0.42 | 0.80 (4/5) | 2.40 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 0.50 | 0.80 (4/5) | 1.00 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」の上でフォーカスが失われ、show_mainが実行されました。<\` |
| log-single-line | logs | 0.44 | 0.80 (4/5) | 2.03 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.95 | yes | `[providers.configs["embedded.llm"]]` |
| markdown-codeblock | markdown | 0.43 | 1.00 (3/3) | 0.70 | yes | ``cargorun--examplebench_specialist_translation--release`を実行してください。レポートはdocs/benc` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.79 | yes | ````bash` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.76 | yes | ````bash` |
| prose-short-greeting | prose | 0.44 | 1.00 (0/0) | 2.77 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 0.50 | 1.00 (0/0) | 1.77 | yes | `こんにたりまworld.<\|im_end>>` |
| prose-idiomatic | prose | 0.40 | 1.00 (0/0) | 2.86 | yes | `速い茶色のキツネが、怠けた犬の上を跳び越える。<\|im_end\|>` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.80 | yes | `速い茶色のキツネが、怠けた犬の上を跳びます。<\|im_end>` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.75 | yes | `速い茶色のキツネが、のんきな犬を飛び越える。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアや実行環境の設定によって性能が異なります。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end>>` |
| prose-meta | prose | 0.42 | 1.00 (0/0) | 1.90 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end(){` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.48 | yes | `小さなデザイナーデルをより大きなメインモデルと相関的に復号する。つまり、デザイナーデルは候補タグの短いシーケンスを自動で予測し、メインモデルは一回の前向きパスで` |
| long-prose-paragraph | long-prose | 0.44 | 1.00 (0/0) | 1.28 | yes | `小さなデザイナーデルをより大きなメインモデルと対応させる推測的デコード方式です。デザイナーは候補トークンを自動的に次予測し、メインモデルは一つの前進処理でその全` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.48 | yes | `小さなデザインモデルをより大きなメインデルと対応付ける推論型デコーィングです。デザインモデルは候補トークンを自動的に次予測し、その後メインデルが一度のフォワード` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.78 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelper` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.50 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelchecks` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 2.37 | yes | `Inaspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelve` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.57 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end(){` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.51 | yes | `스는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end(){` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.80 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end>>` |
| multi-en-to-fr | multilingual | 0.39 | 1.00 (0/0) | 4.47 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 0.50 | 1.00 (0/0) | 1.30 | yes | `MoonLensestunoutildetraductiond'écranbasésurlaconfidentialité,quifonctionneentiè` |
| multi-en-to-fr | multilingual | 0.50 | 1.00 (0/0) | 1.31 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneenti` |

### Hy-MT2 1.8B @ temp=0.2

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップ時間：38ミリ秒（バックエンドカ` |
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.87 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディドLLMのウームアップが完了しました。38ミリ秒（バッ` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップが完了しました。38ミリ秒（バッ` |
| log-single-line | logs | 0.40 | 0.80 (4/5) | 3.57 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」上のフォーカスがshow_main前に失われたtauri::win` |
| log-single-line | logs | 0.42 | 0.80 (4/5) | 2.03 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 0.50 | 0.80 (4/5) | 1.10 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.46 | 1.00 (3/3) | 3.14 | yes | ````bash` |
| markdown-codeblock | markdown | 0.76 | 1.00 (3/3) | 1.97 | yes | ````bash` |
| markdown-codeblock | markdown | 0.76 | 1.00 (3/3) | 4.83 | yes | ````bash` |
| prose-short-greeting | prose | 0.42 | 1.00 (0/0) | 4.38 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-short-greeting | prose | 0.42 | 1.00 (0/0) | 4.38 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-short-greeting | prose | 0.36 | 1.00 (0/0) | 16.23 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.73 | yes | `素早い茶色の狐が、怠け者の犬を飛び越える。<\|im_end>>` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.48 | yes | `素早い茶色の狐が、怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.73 | yes | `素早い茶色の狐が、怠け者の犬を飛び越える。<\|im_end>>` |
| prose-meta | prose | 0.44 | 1.00 (0/0) | 1.13 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end(){` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end>>` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.49 | yes | `小さなデザインモデルをより大きなメインデルと対応付ける定的デコーィングです。デザインモデルは候補トークンを自動的に次予測し、その後メインデルが一回の前向き処理で` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.51 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコードです。デザイナーは候補トークンを自動的に前進的に予測し、その後メインモデルが一回のフォワ` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.51 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコード方式です。デザイナーは候補トークンの短いシーケンスを自動的に次予測し、その後メインモデル` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.53 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.58 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.53 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.80 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end>>` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.80 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end>>` |
| multi-en-to-ko | multilingual | 0.39 | 1.00 (0/0) | 1.33 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end(){` |
| multi-en-to-fr | multilingual | 0.50 | 1.00 (0/0) | 1.34 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 0.35 | 1.00 (0/0) | 8.39 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivéequifonctionn` |
| multi-en-to-fr | multilingual | 0.50 | 1.00 (0/0) | 1.35 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaprotectiondesdonnées,quifonction` |

### Hy-MT2 1.8B @ temp=0.3

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.67 | 1.00 (16/16) | 0.87 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（バックエンドカー` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンデッドLLMのウームアップが完了しました。38ミリ秒（バッ` |
| log-rust-tracing-multi | logs | 0.67 | 0.94 (15/16) | 0.86 | yes | `2026-05-18T00:51:11.674022Zinfoembedded_llm_perf:エンディッドLLMのウームアップ：38ミリ秒（バックエンドカー` |
| log-single-line | logs | 0.42 | 0.80 (4/5) | 3.60 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示前に失われたtauri::window:show` |
| log-single-line | logs | 0.40 | 0.80 (4/5) | 2.67 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」で焦点が表示前に失われたtauri::window:show_ma` |
| log-single-line | logs | 0.42 | 0.80 (4/5) | 2.03 | yes | `2026-05-18T00:51:11ZWARNtauri::window:ラベル「main」でフォーカスが表示前に失われたtauri::window:show` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 0.39 | 1.00 (4/4) | 5.58 | yes | `[providers.configs["embedded.llm"]]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| structured-toml | structured | 1.00 | 1.00 (4/4) | 0.94 | yes | `[providers.configs."embedded.llm"]` |
| markdown-codeblock | markdown | 0.76 | 1.00 (3/3) | 5.99 | yes | ````bash` |
| markdown-codeblock | markdown | 0.45 | 1.00 (3/3) | 3.89 | yes | ````bash` |
| markdown-codeblock | markdown | 0.42 | 1.00 (3/3) | 5.13 | yes | ````bash` |
| prose-short-greeting | prose | 0.42 | 1.00 (0/0) | 4.38 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-short-greeting | prose | 0.42 | 1.00 (0/0) | 4.38 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-short-greeting | prose | 0.42 | 1.00 (0/0) | 4.38 | yes | `こんにちは、世界。<\|im_end(){` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.80 | yes | `素早い茶色のキツネが、怠けた犬の上を跳び越える。<\|im_end>>` |
| prose-idiomatic | prose | 0.44 | 1.00 (0/0) | 1.34 | yes | `素早い茶色の狐が、怠けた犬の上を跳び越える。<\|im_end(){` |
| prose-idiomatic | prose | 0.50 | 1.00 (0/0) | 0.82 | yes | `素早い茶色のキツネが、怠け者の犬の上を跳び越える。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の構成によって性能が異なります。<\|im_end>>` |
| prose-meta | prose | 0.50 | 1.00 (0/0) | 0.61 | yes | `ハードウェアと実行環境の設定によって性能が異なります。<\|im_end>>` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.53 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測型デコードでは、デザイナーは候補トークンの短いシーケンスを自動的に予測し、その後メインモデルが一つの` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 0.54 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける推測的なデコードの仕組みです。デザイナーデルは候補トークンの短いシーケンスを自動的に次予測し、その後メイ` |
| long-prose-paragraph | long-prose | 0.50 | 1.00 (0/0) | 1.28 | yes | `小さなデザイナーデルをより大きなメインモデルと対応付ける定的デコードです。デザイナーは候補トークンを自動的に次予測し、その後メインモデルが一回のフォワードパスで` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.53 | yes | `Inspeculativedecoding,thedraftmodelpredictsthecandidatetokens,andthemainmodelver` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.53 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelverifi` |
| reverse-ja-to-en | reverse | 0.50 | 1.00 (0/0) | 2.50 | yes | `Inspeculativedecoding,thedraftmodelpredictscandidatetokens,andthemainmodelvalida` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.58 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end(){` |
| multi-en-to-ko | multilingual | 0.50 | 1.00 (0/0) | 0.80 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end>>` |
| multi-en-to-ko | multilingual | 0.38 | 1.00 (0/0) | 2.75 | yes | `MoonLens는사용자의로터에서완전히실행되는프라이버시우선화면번역도구입니다.<\|im_end\|>` |
| multi-en-to-fr | multilingual | 0.50 | 1.00 (0/0) | 1.29 | yes | `MoonLensestunoutildetraductiond'écrancentrésurlaconfidentialitéquifonctionneenti` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 1.24 | yes | `MoonLensestunoutildetraductiond'écranaxésurlaprotectiondelavieprivée,quifonction` |
| multi-en-to-fr | multilingual | 0.35 | 1.00 (0/0) | 9.44 | yes | `MoonLensestuneoutildetraductiond'écranaxésurlaconfidentialité,quifonctionneentiè` |

### llm-jp-3.1 1.8B (baseline)

| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |
|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | 0.52 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-rust-tracing-multi | logs | 0.52 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-rust-tracing-multi | logs | 0.52 | 1.00 (16/16) | 0.92 | yes | `2026-05-18T00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compile` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.88 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| log-single-line | logs | 1.00 | 0.60 (3/5) | 0.88 | yes | `2026年5月18日午前0時51分11秒Z警告tauri::window:focuslostonlabel='main'beforeshow_mainroute` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.83 | 1.00 (1/1) | 0.92 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | 0.37 | 1.00 (1/1) | 2.06 | yes | `pubasyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| structured-toml | structured | 0.47 | 0.75 (3/4) | 1.36 | yes | `以下のテキストを日本語に翻訳します。` |
| structured-toml | structured | 0.50 | 0.75 (3/4) | 1.22 | yes | `以下のテキストを日本語に翻訳します。` |
| structured-toml | structured | 0.50 | 0.25 (1/4) | 0.63 | yes | `Qwen2.5-1.5B-Instruct-GGUFリポジトリを使用して、温度0.4、コンテキスト数8192のモデルにコンパイルします。` |
| markdown-codeblock | markdown | 0.71 | 1.00 (3/3) | 0.70 | yes | `ベンチを走らせるには、以下のコマンドを実行してください。` |
| markdown-codeblock | markdown | 0.91 | 1.00 (3/3) | 0.99 | yes | `以下のコードを日本語に翻訳します。` |
| markdown-codeblock | markdown | 1.00 | 1.00 (3/3) | 1.12 | yes | `以下のように翻訳します。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.43 | yes | `素早い茶色の狐が怠け者の犬を飛び越える` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `素早い茶色の狐が怠け者の犬を飛び越える。` |
| prose-idiomatic | prose | 1.00 | 1.00 (0/0) | 0.57 | yes | `素早い茶色のキツネが怠け者の犬の上を飛び越えます。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| prose-meta | prose | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーマンスはハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.45 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせます。このドラフトは候補となるトークンの短いシーケンスを予測し、その後、全体のシークエンスが一度にフォワード` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.54 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて、次のような処理をします。まず、ドラフトは短い候補トークンのシーケンスを予測し、それを繰り返し検証します。` |
| long-prose-paragraph | long-prose | 1.00 | 1.00 (0/0) | 0.46 | yes | `小さなドラフトモデルと大きなメインモデルを組み合わせて、次のように翻訳します。「小さなドラフトモデルは短い候補トークンのシーケンスを予測し、その後、メインモデル` |
| reverse-ja-to-en | reverse | 0.44 | 1.00 (0/0) | 2.85 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelvalid` |
| reverse-ja-to-en | reverse | 0.44 | 1.00 (0/0) | 3.88 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| reverse-ja-to-en | reverse | 1.00 | 1.00 (0/0) | 1.62 | yes | `Inthedecodabletranslation,adraftamodelpredictscandidatetokensandanmainmodelverif` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 7.80 | yes | `ムーンレンズ는strugge한://localhost:8000에의해://이://translated.moonlens.com/translated.htm` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 0.59 | yes | `MoonLens는사용자의로컬컴터에서완전히실행되는strangerfirstscreen翻譯工具입니다.城町` |
| multi-en-to-ko | multilingual | 1.00 | 1.00 (0/0) | 5.51 | yes | `ムーンレンズ는로컬{})()()()()()()()()()()()()()()()()()()()()()()()()()()()()()()()()()()` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.91 | yes | `MoonLensestunoutildetraductionprivéequifonctionneentièrementsurvotremachinelocal` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.91 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |
| multi-en-to-fr | multilingual | 1.00 | 1.00 (0/0) | 0.99 | yes | `MoonLensestunoutildetraductionprivéquifonctionneentièrementsurvotremachinelocale` |


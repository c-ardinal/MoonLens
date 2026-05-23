# Translate Quality Eval (routed)

**Label**: iter6-placeholder-stream-unmask

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 4 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| indented-multiline | ws_score | Standard=0.67 | Compact=0.72 |
| log-rust-tracing-multi | ws_score | Quality=0.13 | Standard=0.28 |
| log-rust-tracing-multi | ws_score | Quality=0.13 | Compact=0.52 |
| log-rust-tracing-multi | ws_score | Standard=0.28 | Compact=0.52 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.90 | 0.74 | 0.89 | 100% | 155.1 | 63/63 |
| Standard | 0.91 | 0.73 | 0.84 | 100% | 155.2 | 63/63 |
| Compact | 0.92 | 0.71 | 0.89 | 100% | 156.9 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.12 | 0.19 (3/16) | 1.57 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.17 | 0.12 (2/16) | 1.15 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.11 | 0.00 (0/16) | 1.61 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.94 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.96 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##表の手順` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 1.02 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.97 | yes | `以下のテキストを日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 1.01 | yes | `以下に日本語で翻します：` |
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
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.53 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.54 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.43 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에연결되지않고,당신의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에연결되지않고,당신의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에서실행되지않고,당신의로컬컴퓨터에만작동합니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `MoonLensestunoutildetraductionenlangueétrangèreprivatiséquifonctionneintégraleme` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `MoonLensestunoutildetraductionenécranprivati-fiablequifonctionneentièrementsurvo` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairespéciali` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairespéciali` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairepluspeti` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéledécodeurpluspet` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesminietprincipal.` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.74 | yes | `Lespeculatifdecodageestuneméthodeconsistantàcombinerdesmodèlesd'apprentissageaut` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.21 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.39 | 0.00 (0/16) | 0.70 | yes | `__MLENS_T_0__:ッシュ・ラーングの初期化：38ミリ秒（コンパイルされたバックエンド・キーード）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.17 | 0.00 (0/16) | 0.80 | yes | `__MLENS_T_0__:ットワークエンジンの初期化：38ミリ秒（コンパイルされたバックエンドのキーング）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.27 | 0.00 (0/16) | 0.68 | yes | `__MLENS_T_0__:ッシュ・LLMの初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.20 (1/5) | 0.74 | yes | `__MLENS_T_0____MLENS_T_1____MLENS_T_2__:__MLENS_T_3__がshow_mainを前に失神した` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.96 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.87 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.62 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.90 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.91 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `速い青いfoxがイジィなdogを跳び越える。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなダーラーモデルと大きな主モデルを組み合わせています：ダーラーは自動回帰的に短い候補トークンのシーンスを予測します、そして主モデルはそのシ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが自動回帰的に短い候補トークンのシーンスを予測し、主モデルはその一貫した` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.43 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.42 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.07 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.56 | yes | `달성렌즈는개인정보보호를우선시한면번역도구이며,부장치에의존하지않고,사용자의로컬컴퓨터에서실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에서실행되지않고,당신의지역컴퓨터에만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `달은나는면번역도구입니다.이도구는당신의로컬컴퓨터에서전히실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneintégralementsurvot` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.94 | yes | `Leméthodededécodagespeculativecombineunepetitemodèled'explorateuretungrandmodèle` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.19 | yes | `Leméthodologiededécodagespeculativeimplémentelacombinaisond'unmodèlededécodeurmi` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodèlededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Décodeurprévisuel:unmodèleprincipalplusgrandestassociéàunmodéleurpluspetitquipré` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.64 | yes | `Lespeculatifdecodageestunmélangedesmodèlesd'apprentissageautomatiquepetitsetlesm` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.33 | yes | `Ladécodagethéoriqueestunetechniquequicombinedesmodèlesdepetitetailleetdesmodèles` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.15 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'inférencepetitsetprincip` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.00 (0/16) | 0.60 | yes | `__MLENS_T_0__:本体のLLM初期化時間:38ms(コンパイルされたバックエンドのキース)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.00 (0/16) | 0.71 | yes | `__MLENS_T_0__:ットワークエンジンの初期化：38ms（コンパイルされたバックエンドキーー）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 0.00 (0/16) | 0.69 | yes | `__MLENS_T_0__:された嵌入型LLMの初期化：38イクロ秒(コンパイルしたバックエンドのケール)` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.20 (1/5) | 0.93 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:show_main:__MLENS_T_5__r` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.40 (2/5) | 0.77 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.90 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:以前に__MLENS_T_4__:ートに焦点を失` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.08 | yes | `こんにちは、世界です。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.81 | yes | `[プロバイダースイシスコ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.85 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.78 | yes | `[プロバイダーズ.構成."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.61 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.58 | yes | `##表の手順` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `##表ノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 0.00 (0/4) | 0.91 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.91 | 0.00 (0/4) | 1.02 | yes | `以下に日本語の翻をごください：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 0.00 (0/4) | 1.01 | yes | `以下に日本語の翻を提供します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.30 | yes | `<p>月Lensに、フォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.57 | yes | `<p__MLENS_T_0__to<strong>月光镜</strong>,はフォーットを尊重する翻ツールです。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `<p__MLENS_T_0__to<strong>月Lens</strong>,はフォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.68 | yes | `キラキラした速いオレンジ色の狐がlazyな犬に跳び越えます。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーンスはハードウェアや実行時設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアと実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.53 | yes | `パフォーンスはハードウェアや実行環境の配置が異なるほど異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.33 | yes | `想的のデコードは、小さなプレーーモデルと大きな主モデルをペアにしており：プレーーが自動回帰的に短い候補トークンのシーンスを予測し、主モデルはその全体シーンスを1` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `スペクulativeデコードは、小さなドライバーモデルと大きな主モデルをペアに組み合わせます：ドライバーが短い候補トークンの自動回帰的なシーンスを予測し、そのシ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `想的解は、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測しますが、主モデルはそのシーンス全` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは次の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `ステップ：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.54 | yes | `deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.97 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.13 | yes | `DecodingwithspeculativedecodingpredictscandidatetokensbyDriftmodel,andaggregates` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.40 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokens,andthemainmodelaggreg` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.70 | yes | `달팽이는개인정보보호를최우선으로하는시creens번역도구입니다.이도구는부트워크에연결되지않고,사용자의로컬신에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.51 | yes | `월령은개인정보보호를최우선으로하는스cren번역도구로,부에연결되지않고로컬신에서실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.54 | yes | `달.oracle은개인정보보호를번째로고려한스cren번역도구입니다.이도구는부장비에서실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `MoonLensestunoutildetraductionparécranàprivacitépremièrequifonctionneentièrement` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.47 | yes | `MoonLensestunoutildetraductionàl'écranquiseconcentresurlaprotectiondesdonnéesper` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductiond'écranprivati-fèrequifonctionneentièrementsurvotr` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.85 | yes | `Leméthodededécodagespeculativecombineunmodèlemodulieretunmodèleprincipalplusgran` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `Lemécanismededécodagespeculativeimplémentelacombinaisond'unmodèlemodulierpetitet` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminiateuretd'unmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.23 | yes | `Décodagespeculativeestunetechniquequiassocieunmodéleprincipalplusgrandavecunmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.84 | yes | `Décodagespeculativeestunetechniquequiassocieunmodèleprincipalplusgrandavecunmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.18 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandavecunmodèlededécodeurpluspe` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.74 | yes | `Ladécodageparinvestissementestuneméthodeconsistantàcombinerunmodèleminiévrededro` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.95 | yes | `Ladécodageàl'effetdepariestuneméthodecombinantunmodèleminièred'unetaillepetiteet` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.44 | yes | `Ladécodageparinvestiestunetechniquecombinantdesmodèlesdetypemini-dragontetmodèle` |
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


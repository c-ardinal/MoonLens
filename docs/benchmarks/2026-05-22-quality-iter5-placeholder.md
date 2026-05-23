# Translate Quality Eval (routed)

**Label**: iter5-placeholder-masking

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 2 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| log-rust-tracing-multi | ws_score | Quality=0.39 | Compact=0.62 |
| log-rust-tracing-multi | ws_score | Standard=0.10 | Compact=0.62 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Standard | 0.90 | 0.68 | 0.89 | 100% | 171.1 | 63/63 |
| Quality | 0.91 | 0.68 | 0.87 | 100% | 171.3 | 63/63 |
| Compact | 0.89 | 0.66 | 0.92 | 100% | 168.8 | 60/63 |

## Per-UserTier × fixture detail

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 0.00 (0/16) | 1.52 | yes | `__MLENS_T_0__:本体のLLM初期化:38ms(コンパイルされたバックエンドのキーング)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 0.00 (0/16) | 1.42 | yes | `__MLENS_T_0__:ンコーーの初期化:38イクロ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.11 | 0.00 (0/16) | 1.66 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1__と__MLENS_T_2__)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__routeran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__routeran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 1.10 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__ートが走っていた前に_` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 1.08 | yes | `[プロバイダーンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 1.02 | yes | `[プロバイダーンフィギュレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 1.00 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.60 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.63 | yes | `##リースノート` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.62 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下の文を日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.90 | yes | `以下は日本語の翻です：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下に日本語で翻します：` |
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
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自作リバースプロセスで短い候補トークンのシーンスを予測します。その後` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが短い候補トークンの自動回帰的なシーンスを予測し、その後主モデルはその一` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせます：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルはそ` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.61 | 1.00 (0/0) | 0.31 | yes | `Deployステップ：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.48 | yes | `部署ステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.68 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.05 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.12 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokensandmainmodelaggregatesmat` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에연결되지않고,당신의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.51 | yes | `달은을차단하는우선번역도구입니다.이도구는부에연결되지않고,당신의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `달은을차단하는사처럼보이는면번역도구입니다.이도구는당신의지역컴퓨터에서전히실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneintégralementsurvot` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductiond'écranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.19 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneentièrementsurvotrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.98 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodologiededécodagespeculativeimpliquelacombinaisond'unmodèlemodérateuretd'` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairebasiquee` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodèlededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesminietprincipal.` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.67 | yes | `Lemélangedelaspécificitédudécodageavecunmodèleprincipalpluslarge.` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.51 | yes | `Lespeculatifdecodageestuneméthoded'ajoutdemodèlesdédiésauxpetitsetunmodèleprinci` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.93 | yes | `SpeculativedecodingassociierteinemkleinenRahmenmodellmiteinemgrößerenHauptmodell` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.92 | yes | `Speculativedecodingassociierteinekleineregrundmodellmiteinergrösserenhauptmodell` |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 0.00 (0/16) | 0.74 | yes | `__MLENS_T_0__:ンコーーの初期化：38イクロ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 0.00 (0/16) | 0.71 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.13 | 0.00 (0/16) | 1.56 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミリ秒(__MLENS_T_1____MLENS_T_2__)` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__routeran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__routeran` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/5) | 0.82 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:__MLENS_T_4__routeran` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.95 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,192` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 0.98 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 1.01 | yes | `[プロバイダーンフィグレーションs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 1.04 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.58 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 1.01 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下の文を日本語に翻します：` |
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
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.36 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.36 | yes | `想的の解は、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルはその一` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.53 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.08 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `달은나는면번역도구입니다.이도구는당신의로컬컴퓨터에서전히실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `달은을보호하는번째면번역도구입니다.이도구는부에서실행되지않고,당신의지역컴퓨터에만작동합니다.` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `달은을보호하는번째면번역도구입니다.이도구는부에연결되지않고,당신의지역컴퓨터에서만실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductionenlangueétrangèrequifonctionneentièrementsurvotrem` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneintégralementsurvot` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairepluspeti` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.17 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairepluspeti` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireélémenta` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurdedécodageplu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.33 | yes | `Ladécodagethéoriqueestunetechniquequicombinedesmodèlesdepetitetailleetdesmodèles` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesminietprincipal.` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.21 | yes | `Ledécodagespeculatifestunmélangedemodèlesd'apprentissageautomatiquepetitsetprinc` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.93 | yes | `SpeculativedecodingassociierteinemkleinenrauhenModellmiteinemgrößerenHauptmodell` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.74 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.00 (0/16) | 0.75 | yes | `__MLENS_T_0__:層学習モデル（LLM）の初期化：38ミリ秒（コンパイルされたバックエンドのケール）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.00 (0/16) | 0.70 | yes | `__MLENS_T_0__:本体のLLM初期化:38ms(コンパイルされたバックエンドのニュールネットワーク)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.00 (0/16) | 0.68 | yes | `__MLENS_T_0__:ンコーーの初期化：38ミルース（コンパイルされたバックエンドのキーニアル）` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 1.13 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:が視線を__MLENS_T_4__に移す前に__` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.99 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:何も視線が向けられていなくて、__MLENS_T` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/5) | 0.91 | yes | `__MLENS_T_0__:__MLENS_T_1__:__MLENS_T_2__:__MLENS_T_3__:以前に__MLENS_T_4__ルートを焦点から` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.95 | yes | `asyncfn__MLENS_T_0__(self:&mutSelf,args:TranslateArgs)->Result<TaskResult,Provid` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.89 | yes | `asyncfn__MLENS_T_0__(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.40 | 0.75 (3/4) | 1.10 | yes | `使用方法：` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.75 (3/4) | 0.96 | yes | `使用方法:` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.75 (3/4) | 0.96 | yes | `Usage:moonlens[--profileNAME][--configPATH][--verbose]capture__MLENS_T_0__0,0,19` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.48 | 0.00 (0/3) | 0.13 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.50 (2/4) | 0.94 | yes | `[プロバイダーの設定."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.50 (2/4) | 1.00 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.25 (1/4) | 1.06 | yes | `Providers.configs."embedded.llm"のリポジトリーは、Qwen/Qwen2.5-1.5B-Instruct-GGUFです。温度は__` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##行情報` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.61 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.59 | yes | `##表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 0.00 (0/4) | 0.92 | yes | `以下の文を日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 0.00 (0/4) | 0.91 | yes | `以下の文を日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 0.00 (0/4) | 0.95 | yes | `以下は日本語の翻です：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.38 | yes | `<p>MoonLens,whichrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.65 | yes | `パフォーンスはハードウェアや実行時間のコンフィギュレーションによって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `パフォーンスはハードウェアや実行時設定に依存します。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーンスはハードウェアと実行時設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドラフトモデルと大きな主モデルを組み合わせています：ドラフトは自作回帰的に短い候補トークンのシーンスを予測します、そして主モデルはその一` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.36 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルをペアにします。ドライバーは自動回帰的に短い候補トークンのシーンスを予測し、その後主モデルはその一連のシ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルをペアで使用します：ドライバーは自動回帰的に短い候補トークンのシーンスを予測し、主モデルはそのシーンス全` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `ステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.70 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesmat` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.60 | yes | `Inspeculativedecoding,Driftmodelpredictscandidatetokens,andMainmodelaggregatesma` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.17 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `달Luna는개인보호를최우선으로하는린번역도구이며,부에서실행되지는않습니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `달은티포인트우선면번역도구입니다.이도구는부저장소에서비롯된정보를사용하지않습니다.MoonLens는당신의지역컴퓨터에전적으로실행됩니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.52 | yes | `달.oracle는프라이버시를우선으로하는면번역도구입니다.이도구는부장비에서실행되지않습니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductionenimageprivatiséquiexécuteentièrementsurvotremachi` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.23 | yes | `MoonLensestunoutildetraductionparécranquisebasesurlaconfidentialitéetfonctionnee` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.08 | yes | `MoonLensestunoutildetraductionenécranbasésurlavieprivéequisedérouleentièrementsu` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.27 | yes | `Leméthodededécodagespeculativeimplémenteunecombinaisond'unepetitemodélisationdra` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminiatureetd'unmodè` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Lemélangedelaméthoded'interprétationspeculativeavecunepetitemodèledécodeuretungr` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.81 | yes | `Décodagespeculativeestunetechniquequiassocieunmodèleprincipalplusgrandavecunmodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.27 | yes | `Décodagespeculativeestuneméthodequiassocieunmodèleprincipalpluslargeavecunmodèle` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.29 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdecandidatsplu` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.77 | yes | `Ladécodageparinvestissementestuneméthodeconsistantàcombinerdesmodèlesdetraîneurm` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 3.21 | yes | `Ladécodageparstratégiedepariestuneméthodequiconsisteàcombinerunmodèleminiaturisé` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 3.10 | yes | `Ladécodageàl'effetdepariestuneméthodecombinantunmodèleminiaturisépourlespetitsob` |
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


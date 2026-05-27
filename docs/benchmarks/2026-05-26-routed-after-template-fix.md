> ⚠️ **2026-05-27 INVALID DATA WARNING** — This report was generated
> BEFORE the routing bug fix (commit `5da7316`,
> `fix(translate): route per-tier filename_remote through to model_path`).
> At the time, `RoutedTranslator` config emitted `filename_remote`
> for each tier but `EmbeddedLlmTranslator::new` ignored it and fell
> back to `DEFAULT_MODEL_FILE = "qwen2.5-1.5b-instruct-q4_k_m.gguf"`,
> so **every tier in this measurement actually ran qwen2.5-1.5b**,
> not the registered tier model (Hy-MT2 / llm-jp / Gemma). The
> per-tier score differences captured here reflect only per-tier
> sampling/MBR-N changes on top of qwen2.5-1.5b — NOT the routing
> matrix's intended model selection. See
> `docs/benchmarks/2026-05-27-routed-after-filename-fix.md` for the
> post-fix numbers that actually exercise the routed models.

# Translate Quality Eval (routed)

**Samples per fixture**: 3

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 5 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| html-tags | id_score | Quality=0.00 | Compact=0.33 |
| html-tags | id_score | Standard=0.00 | Compact=0.33 |
| log-rust-tracing-multi | ws_score | Quality=0.28 | Standard=0.43 |
| log-rust-tracing-multi | ws_score | Quality=0.28 | Compact=0.56 |
| log-rust-tracing-multi | ws_score | Standard=0.43 | Compact=0.56 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Compact | 0.92 | 0.91 | 0.85 | 100% | 121.1 | 60/63 |
| Quality | 0.91 | 0.90 | 0.86 | 100% | 0.7 | 63/63 |
| Standard | 0.92 | 0.90 | 0.82 | 100% | 1.4 | 63/63 |

## Per-UserTier × fixture detail

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.50 | 0.94 (15/16) | 0.75 | yes | `2026-05-18T00:51:11.674022Z:クスチャ・レンダリング・エンジンの初期化：38ミリ秒（コンパイルされたバックエンドキーード）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.52 | 0.94 (15/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:モデルの初期化:38イクロ秒(コンパイルされたバックエンドのケール)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.81 (13/16) | 0.66 | yes | `2026-05-18T00:51:11.674022Z:ットワークエンジンの初期化：38ミリ秒（コンパイルされたバックエンドのニュールネットワークのキーード）` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.74 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:label='main':show_mainrouteran` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.82 | yes | `2026-05-18T00:51:11ZWARNtauri::window:label='main'の前にshow_mainルートが焦点から逸脱しました。` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (5/5) | 0.87 | yes | `2026-05-18T00:51:11Z:WARN:tauri::window:途中でlabel='main'を追いつくことができずにshow_main線に走っ` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.90 | yes | `asyncfnrun_translate(args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.83 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 1.00 | yes | `[Providers.configs."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.72 | yes | `[提供者.配置文件."嵌入的llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.91 | 1.00 (4/4) | 0.97 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.89 | yes | `以下は日本語の翻です：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 1.04 | yes | `以下のテキストを日本語に翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.38 | yes | `<p>MoonLens,whichrespectsformatting.</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (1/1) | 0.66 | yes | `<pclass="intro">Welcometo<strong>月光镜</strong>,一致するフォーットを尊重する翻ツール。</p>` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.52 | yes | `<p>MoonLens,atranslationtoolthatrespectsformatting.</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.68 | yes | `quickbrownfoxjumpsoverlazydog.` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `quickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアと実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的のデコードは、小型のドラフトモデルと大型の主モデルを組み合わせています。ドラフトは過去の候補トークンの短い序列を自作リバースプロセスとして予測し、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.35 | yes | `想的のデコードは、小さなディレクターモデルと大きな主モデルを組み合わせています：ディレクターが短い候補トークンの自動回帰的な順序を予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的で解は、小型のデッキーードルと大型の主モードを組み合わせています：デッキーードルは、自動回帰的に短い候補トークンのシーンスを予測し、主モードはそのシーンス全` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.61 | 1.00 (0/0) | 0.37 | yes | `展開ステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.57 | yes | `deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (0/0) | 0.50 | yes | `Deployステップは次のとおりです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.33 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.68 | yes | `Inthespeculativedecoding,DRAFTpredictscandidatetokensandMaincombinesmatchinghead` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.57 | yes | `Inspeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregatesma` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `월렌스는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `달팽이는사용자의로컬컴퓨터에서전히실행되는프라이버시우선면번역도구입니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductiondefenêtreprivatiséquifonctionneintégralementsurvot` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `MoonLensestunoutildetraductiond'écranàprivacitéenavant-gardequifonctionneentière` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `MoonLensestuneapplicationdetraductiond'écranprivéequifonctionneintégralementsurv` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèledrafteurpetitetd'un` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairepetitetu` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.93 | yes | `Leméthodededécodagespeculativecomportel'ajoutd'unmodèleminièreetd'unmodèleprinci` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandàunmodéleurd'entréepluspetit` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedébuts.Lemod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.97 | yes | `Décodagespeculativeassocieunmodèleprincipalplusgrandàunmodélepilote:ledrafterpré` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.67 | yes | `Ladécodageparinvestissementestuneméthodecombinantunmodèleminiaturisédedroneetunm` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.46 | yes | `Ladécodageparinvestissementcomportelacombinaisond'unmodèleminidedroneetd'unmodèl` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.54 | yes | `Ladécodageàl'effetde"jouer"estunetechniquecombinantdesmodèlesd'inférenceminietmo` |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38イクロ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.17 | 1.00 (16/16) | 1.32 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.17 | 1.00 (16/16) | 1.30 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
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
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `[プロバイダーズ.コンフィグレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.58 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下に日本語で翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
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
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.44 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルは` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています。ドライバーは、自作リバースプロセスで短い候補トークンのシーンスを予測します。その後` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.44 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.99 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireetd'unmo` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.12 | yes | `Leméthodologiededécodagespeculativeimpliquelacombinaisond'unmodèlemodérateuretd'` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.49 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesd'apprentissageautomatiqu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.85 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesminietprincipal.<` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 0.94 (15/16) | 0.63 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38イクロ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.10 | 1.00 (16/16) | 1.61 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.52 | 0.94 (15/16) | 0.70 | yes | `2026-05-18T00:51:11.674022Z:ッシュの初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
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
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.91 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.86 | yes | `[プロバイダーズ.コンフィギュース."embedded.llm"]` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.83 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.60 | yes | `##表の記` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##表の詳細` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (1/1) | 0.59 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.92 | yes | `以下のテキストを日本語に翻します：` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
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
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行環境の設定によって異なります。` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.35 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーが自動回帰的に短い候補トークンのシーンスを予測し、その後主モデルはそのシ` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.37 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデル` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `想的のデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは自動回帰的に短い候補トークンのシーンスを予測しますが、主モデルはその全` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (0/0) | 0.46 | yes | `Deployステップは以下の通りです：` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.56 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.90 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.68 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.72 | yes | `Inspeculativedecoding,DriftmodelpredictscandidatetokensandMainmodelaggregatesthe` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `달은나는번역도구입니다.이번역도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.39 | yes | `달은나는번역도구입니다.이도구는사용자의로컬컴퓨터에서전히실행됩니다.<` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.96 | yes | `MoonLensestunoutildetraductiond'écranprivéquifonctionneentièrementsurvotremachin` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `MoonLensestunoutildetraductionenécranprivatisantquifonctionneentièrementsurvotre` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `MoonLensestunoutildetraductionparécranprivatisantquifonctionneentièrementsurvotr` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.18 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulairepluspeti` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.04 | yes | `Leméthodologiededécodagespeculativeimplémentelacombinaisond'unmodèlemodulaireetd` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.01 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodulieretd'unmodèl` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.13 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodélededécodageplusp` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.51 | yes | `Ledécodagethéoriqueestuneméthodedecombinaisondemodèlesd'apprentissagepetitetmodè` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.82 | yes | `Ledécodagethéoriqueestunmélangedemodèlesd'inférencepetitsetprincipaux.<` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.62 | yes | `Lemélanged'unmodèlededécodagemodéréavecunmodèleprincipallarge.<` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `Speculativedecodingassociatesasmallroughmodelwithalargermainmodeltoaccelerategen` |

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


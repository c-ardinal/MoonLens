# Translate Quality Eval (routed)

**Label**: after-1B-3A-3B

**Samples per fixture**: 1

**Hardware**: Gpu

**Fixtures evaluated**: 21

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Standard | 0.94 | 0.84 | 0.83 | 100% | 154.5 | 21/21 |
| Compact | 0.93 | 0.83 | 0.88 | 100% | 129.9 | 20/21 |
| Quality | 0.90 | 0.83 | 0.87 | 100% | 160.3 | 21/21 |

## Per-UserTier × fixture detail

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.60 (3/5) | 0.89 | yes | `2026年5月18日00:51:11ーストtauri::window:mainのlabel='main'を表示前のshow_mainrouteでフォースが失われ` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.93 | yes | `asyncfnrun_translate(self,args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.84 | yes | `使用方法:moonlens[--profile名前][--configス][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.84 | yes | `[プロバイダーズ.コンフィグ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.57 | yes | `##表ノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.87 | yes | `台を回すと、以下のようになります：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.45 | yes | `<pclass="intro">月レンズへようこそ。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.82 | yes | `Thequickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `パフォーンスはハードウェアや実行時設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.47 | yes | `想的なデコードは、小さなドラフラーモデルと大きな主モデルを組み合わせています：ドラフラーが自動回帰的に短い候補トークンの序列を予測し、その後主モデルはその全体的` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.49 | yes | `Deployステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.87 | yes | `Inspeculativedecoding,Drafthmodelpredictscandidatetokensandthemainmodelaggregate` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `달.oracle은프라이버시-first면번역도구로,모든기기에서실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.98 | yes | `MoonLensestunoutildetraductionenlanguevisiblequifonctionneentièrementsurvotremac` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.95 | yes | `Leméthodededécodagespeculatifcomportelacombinaisond'unmodèlemodulaireetd'unmodèl` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.11 | yes | `Décodeurprévisuel:unmodèleprincipalplusgrandestassociéàunmodèlededécodeurpluspet` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.03 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesminietdemodèleprincipal.` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.76 | yes | `SpeculativedecodingverknüpftmalsmallenDraughtmodellmiteinemgrößerenMainmodellobe` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.60 (3/5) | 0.76 | yes | `2026年5月18日00:51:11WARNtauri::window:mainラベルのフォースが前のshow_mainルーンで失われました。` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.97 | yes | `asyncfnrun_translate(self:&mutSelf,args:TranslateArgs)->Result<TaskResult,Provid` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.48 | 0.00 (0/3) | 0.13 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.84 | yes | `[プロバイダーズ.コンフィグス."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.57 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.99 | yes | `以下に日本語の翻を示します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.45 | yes | `<pclass="intro">月レンズへようこそ。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `快速のい狐がlazyの犬に跳び越えます。` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.45 | yes | `パフォーンスはハードウェアや実行環境によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.38 | yes | `特定的解は、小さなドライバーモデルと大きな主モデルをペアにしており：ドライバーが自動回帰的に短い候補トークンのシーンスを予測し、主モデルはその一貫したシーンス全` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.55 | yes | `ステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.38 | yes | `Inspeculativedecoding,theDriftmodelpredictscandidatetokensandthemainmodelaggrega` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.57 | yes | `달Luna는개인隐私를보장하는치번역도구입니다.이도구는모든작업이localmachine에서실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.00 | yes | `MoonLensestunoutildetraductionenimageprivilégiéquifonctionneentièrementsurvotrem` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.06 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlemodérateuretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodeurspeculativecombineunmodéleprincipalplusgrandavecunmodèled'entréepluspeti` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.38 | yes | `Ladécodageparinvestissementestuneméthodecombinantunmodèlededroneminietunmodèlepr` |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.60 (3/5) | 0.83 | yes | `2026年5月18日(土)0:51:11ZWARNtauri::window:mainラベルにフォースが失われた前にshow_mainルートが実行されました` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.93 | yes | `asyncfnrun_translate(self,args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.40 | 1.00 (4/4) | 1.29 | yes | `月光レンズを使用する際、以下のように指定してください。` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.75 (3/4) | 0.83 | yes | `[プロバイダーンフィグレーションs."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.61 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.87 | yes | `以下は日本語の翻です：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.50 | yes | `<pclass="intro">月光鏡（まつきがね）へようこそ。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.40 | yes | `speculativedecodingは、小型のデラーモデルと大型の主モデルを組み合わせています：デラーは自作リニア性で短い候補トークンのシーンスを予測しますが` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.52 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.73 | yes | `Inthespeculativedecoding,DrafthmodelpredictscandidatetokensandMainmodelaggregate` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.46 | yes | `달은나는면번역도구입니다.이도구는부에연결되지않고,당신의로컬컴퓨터에서만실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.05 | yes | `MoonLensestuneapplicationdetraductionenécranprivéequifonctionneintégralementsurv` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.07 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèlededécodeurpetitetd'` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.10 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéled'entréepluspet` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.33 | yes | `Ledécodagespeculatifestuneméthodedecombinaisondemodèlesdepetitetailleetdemodèles` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.81 | yes | `SpeculativedecodingassociierteinemalsmalgernenModellmitmehrerganzenModellumfängt` |

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


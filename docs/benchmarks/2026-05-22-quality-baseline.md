# Translate Quality Eval (routed)

**Label**: baseline-before-quality-push

**Samples per fixture**: 1

**Hardware**: Gpu

**Fixtures evaluated**: 21

## Invariant check: ❌ 18 VIOLATIONS (0.05-tolerance)

Upper tiers are scoring worse than lower tiers on these (fixture × metric) cells. Each row must be eliminated by a targeted fix (sampling tweak, routing change, output post-process, or model swap).

| Fixture | Metric | Upper (worse) | Lower (better) |
|---|---|---|---|
| code-cli-help | length_ratio_dev | Standard=0.84 | Compact=0.89 |
| debug-tech-en-to-fr-1 | length_ratio_dev | Quality=0.86 | Standard=0.98 |
| debug-tech-en-to-fr-1 | length_ratio_dev | Quality=0.86 | Compact=0.97 |
| html-tags | length_ratio_dev | Quality=0.45 | Compact=0.62 |
| html-tags | length_ratio_dev | Standard=0.49 | Compact=0.62 |
| log-rust-tracing-multi | id_score | Quality=0.75 | Standard=1.00 |
| log-rust-tracing-multi | id_score | Quality=0.75 | Compact=1.00 |
| log-single-line | id_score | Standard=0.40 | Compact=0.60 |
| markdown-bullets | length_ratio_dev | Quality=0.56 | Compact=0.63 |
| markdown-codeblock | ws_score | Quality=0.83 | Standard=1.00 |
| multi-en-to-fr | length_ratio_dev | Quality=0.91 | Compact=0.98 |
| multi-en-to-fr | length_ratio_dev | Standard=0.78 | Compact=0.98 |
| multi-en-to-ko | length_ratio_dev | Quality=0.42 | Standard=0.57 |
| multi-en-to-ko | length_ratio_dev | Quality=0.42 | Compact=0.62 |
| multi-en-to-ko | length_ratio_dev | Standard=0.57 | Compact=0.62 |
| reverse-ja-to-en | length_ratio_dev | Standard=-0.60 | Compact=0.00 |
| structured-toml | id_score | Standard=0.50 | Compact=1.00 |
| structured-toml | length_ratio_dev | Standard=0.81 | Compact=0.97 |

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Compact | 0.92 | 0.83 | 0.88 | 100% | 130.2 | 20/21 |
| Quality | 0.93 | 0.83 | 0.85 | 100% | 167.4 | 21/21 |
| Standard | 0.94 | 0.80 | 0.90 | 100% | 163.7 | 21/21 |

## Per-UserTier × fixture detail

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.60 (3/5) | 0.77 | yes | `2026年5月18日00:51:11WARNtauri::window:mainベルが表示される前にshow_main線でフォースを失いました。` |
| code-rust-signature | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (1/1) | 0.93 | yes | `asyncfnrun_translate(self,args:TranslateArgs)->Result<TaskResult,ProviderError>{` |
| code-cli-help | code | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (4/4) | 0.97 | yes | `[プロバイダーズ.コンフィギュレーションズ."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.63 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.83 | 1.00 (4/4) | 0.90 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 0.00 (0/1) | 0.62 | yes | `<pclass="intro">こんにちは、<strong>月レンズ</strong>。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアや実行時間の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.35 | yes | `想的の解は、小さな抽選モデルと大きな主モデルを組み合わせています。抽選モデルは、自動回帰的に短い候補トークンのシーンスを予測します。その後、主モデルはその一貫し` |
| indented-multiline | indented | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.72 | 1.00 (0/0) | 0.50 | yes | `Deployステップは次の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.00 | yes | `Inspeculativedecoding,Drafthmodelpredictscandidatetokensandthemainmodelaggregate` |
| multi-en-to-ko | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.62 | yes | `달팽이는개인정보보호를최우선으로하는면번역도구입니다.이도구는부에서실행되지않고,사용자의로컬컴퓨터에만실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.98 | yes | `MoonLensestunoutildetraductionenimageprivéequifonctionneintégralementsurvotremac` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.03 | yes | `Leméthodededécodagespeculativecomportelacombinaisond'unmodèleminièreetd'unmodèle` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.20 | yes | `Décodeurspeculativeassocieunmodèleprincipalplusgrandavecunmodéleurdedécodageplus` |
| pivot-ja-to-fr | multilingual | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 2.38 | yes | `Ladécodageparinvestissementestuneméthodecombinantunmodèlededroneminietunmodèlepr` |
| pivot-fr-to-de | multilingual | `Unsupported` | — | — | — | — | _Unsupported_ |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 0.75 (12/16) | 0.91 | yes | `2026年5月18日00:51:11.674022Z INFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.60 (3/5) | 0.74 | yes | `2026年5月18日00:51:11WARNtauri::window:ベル'main'を表示する前にshow_main線が実行されました。` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.97 | yes | `asyncfnrun_translate(self:&mutSelf,args:TranslateArgs)->Result<TaskResult,Provid` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.89 | yes | `使用方法:moonlens[--profileNAME][--configPATH][--verbose]capture--region0,0,1920,108` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.95 | yes | `[provider.config."embedded.llm"] ` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.56 | yes | `##リースノート` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (4/4) | 0.88 | yes | `以下に日本語で翻します：` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.45 | yes | `<pclass="intro">月レンズへようこそ。フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.75 | yes | `quickbrownfoxjumpsoverthelazydog.` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.50 | yes | `パフォーンスはハードウェアとランタイム設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.41 | yes | `スペクulativeデコードは、小さなドライバーモデルと大きな主モデルを組み合わせています：ドライバーは過去の一部の候補トークンの短いシーンスを自動回帰的に予測` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.52 | yes | `Deployステップは以下の通りです：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 1.53 | yes | `Inspeculativedecoding,DRAFTpredictscandidatetokensandMainmergestheheadwordsofmat` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.42 | yes | `달은나는면번역도구로알려져있으며,그모든기능이사용자의로컬컴퓨터에실행됩니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.09 | yes | `MoonLensestunoutildetraductionauniveauduclavierprivéequifonctionneintégralements` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.14 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlededécodeurminuscu` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.22 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleurpilotepluspet` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.28 | yes | `Ledécodagespeculatifestuneméthodecombinantdesmodèlesdepetitetailleetdesmodèlespr` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e4b-it-q4_k_m→gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.85 | yes | `Speculativedecodingassociiertsmallsketchmodellmitlargemainprinzipalmodellumvervo` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.67 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022ZINFOembedded_llm_perf:embeddedLLMwarmup:38ms(compiled` |
| log-single-line | logs | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.40 (2/5) | 0.78 | yes | `2026年5月18日00:51:11ォルトtauri::window:mainベルのフォースが前のshow_mainートに達した後に失われました。` |
| code-rust-signature | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.83 | 1.00 (1/1) | 0.96 | yes | `asyncfnrun_translate(self:*Self,args:TranslateArgs)->Result<TaskResult,ProviderE` |
| code-cli-help | code | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.84 | yes | `使用方法：moonlens[--profile名前][--configス][--verbose]capture--region0,0,1920,1080` |
| structured-json | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.43 | 0.00 (0/3) | 0.06 | yes | `こんにちは、世界。` |
| structured-toml | structured | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.50 (2/4) | 0.81 | yes | `[プロバイダーンフィグレーション."embedded.llm"]` |
| markdown-bullets | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.58 | yes | `###表の詳細` |
| markdown-codeblock | markdown | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (4/4) | 0.91 | yes | `Runthebenchwith:` |
| html-tags | html | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 0.00 (0/1) | 0.49 | yes | `<pclass="intro">MoonLensへようこそ、フォーットを尊重する翻ツールです。</p>` |
| prose-short-greeting | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.69 | yes | `こんにちは、世界。` |
| prose-idiomatic | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.66 | yes | `quickbrownfoxjumpsoverlazydog` |
| prose-meta | prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.48 | yes | `パフォーンスはハードウェアと実行時の設定によって異なります。` |
| long-prose-paragraph | long-prose | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 0.36 | yes | `想的な解は、小さなモデルと大きな主モデルを組み合わせています：小家が一連の候補トークの短いシーンスを自作リバースプロセッシングで予測し、主モデルはその一連のシー` |
| indented-multiline | indented | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 0.72 | 1.00 (0/0) | 0.51 | yes | `Deployステップ：` |
| reverse-ja-to-en | reverse | `Direct:llm-jp-3.1-1.8b-instruct4-q4_k_m` | 1.00 | 1.00 (0/0) | 2.60 | yes | `Inspeculativedecoding,theDriftModelpredictscandidatetokens,andthemainmodelaggreg` |
| multi-en-to-ko | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.57 | yes | `달은나는면번역도구로알려져있으며,부에의존하는것이없습니다.이를통해모든사용자가원활하게번역할수있습니다.` |
| multi-en-to-fr | multilingual | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.22 | yes | `MoonLensestunoutildetraductionparécranàfaibleimpactsurlavieprivéequifonctionneex` |
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.02 | yes | `Leméthodededécodagespeculativeimplémentelacombinaisond'unmodèlemodulieretd'unmod` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 1.21 | yes | `Décodeurspeculative:unmodèleprincipalplusgrandestassociéàunmodéleprédecesseurplu` |
| pivot-ja-to-fr | multilingual | `Pivot:llm-jp-3.1-1.8b-instruct4-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 2.41 | yes | `Latechniquededécodagespeculatifcombineunmodèleprincipalplusgrandavecunmodèledéco` |
| pivot-fr-to-de | multilingual | `Pivot:gemma-4-e2b-it-q4_k_m→gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.87 | yes | `SpeculativedecodingverknüpftsmitigsmalschmalenVorlärmodelsmitgroßenMainmodelleib` |

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


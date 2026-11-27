# 画面文字による音声認識補強の効果測定

ウィンドウ翻訳が読取した画面文字（人名・地名・用語）を、音声認識（内蔵 whisper.cpp）へ
語彙プロンプトとして供給した場合の認識精度を測定。製品化判断の根拠。

## 方式

- 音声：[FLEURS](https://huggingface.co/datasets/google/fleurs)（Google、CC BY 4.0）dev 分割の日本語・英語。
  固有名詞または専門用語を含む文（日本語 67 文、英語 81 文）を対象とし、各文の先頭話者 1 件を使用。
- 注釈：`terms.<lang>.tsv`（文 ID と、原文に含まれる固有名詞・用語）。
- 認識：既定モデル（Whisper large-v3-turbo q8_0）、言語は自動判定（アプリ既定と同一）。
- 条件（＝音声認識へ供給するプロンプト）：

| 条件 | プロンプト | 想定場面 |
|---|---|---|
| `none` | 無し | 現行動作 |
| `oracle` | 文中の用語のみ | 上限値 |
| `mixed` | 文中の用語＋無関係な用語 15 件（順序は乱択） | 情報量の多い画面 |
| `distract` | 無関係な用語 15 件のみ | 画面と音声が無関係 |
| `cross` | 同一文の他言語側の用語 | 画面が日本語字幕、音声が英語（逆も同様） |

- 指標：
  - 誤り率：日本語は CER（文字単位）、英語は WER（単語単位）。句読点・空白・大小文字・全半角の差は無視。
  - 用語再現：注釈した用語が原文と同一表記で出力された割合。
  - 混入：無関係な用語が、発話に無いにも関わらず出力へ出現した文の割合。
  - 別言語：音声と異なる言語で出力された文の割合。
  - 差の区間：`none` との対応有りブートストラップ（95%、文単位 1 万回）。
- 分割入力：`--chunk-seconds 3` は、ライブ字幕と同様に音声を 3 秒単位（0.5 秒の重複付き）へ分割し、
  各チャンクを同一プロンプトで個別認識。重複部分の再出力により誤り率の絶対値は上昇するため、条件間の差のみ有効。

## 実行

```sh
python3 bench/asr-bias/prepare.py work            # 音声と認識ジョブの生成
cd src-tauri
cargo run --release --example asr_bias_bench -- ../work/ja/jobs.jsonl ../work/ja/results.jsonl --fetch
cargo run --release --example asr_bias_bench -- ../work/en/jobs.jsonl ../work/en/results.jsonl --fetch
cd ..
python3 bench/asr-bias/score.py work/ja work/en
```

CI：`.github/workflows/asr-bias.yml`（本ディレクトリ変更時のみ実行、GPU 無しで 1 ジョブ約 2 時間）。
結果はジョブ概要と成果物 `asr-bias-*` に出力。

## 制約

- FLEURS は百科事典文の読上。ゲーム・アニメの造語は対象外（Whisper の未知語であり、効果は本測定より大きい可能性）。
- プロンプトは正確な表記を使用。OCR の誤読を含む場合の効果は未測定。
- 用語注釈は人手 1 名分。

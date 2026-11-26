# 音声フィクスチャ

- `jfk.wav`：ジョン・F・ケネディ大統領就任演説（1961 年）の抜粋、11 秒、16 kHz・モノラル・PCM16。
  米国政府の著作物（パブリックドメイン）。whisper.cpp の `samples/jfk.wav` と同一。
  用途：`tests/subtitle_live.rs`（英語音声 → 認識 → 翻訳の一貫確認）。
- `ja-moldova.wav`：日本語の一文「モルドバの主な宗教は正統派キリスト教です。」、6 秒、16 kHz・モノラル・PCM16。
  出典：[FLEURS](https://huggingface.co/datasets/google/fleurs)（Google、CC BY 4.0）の日本語 dev 分割
  `2493802856426045210.wav`。変更点：前後の無音除去、PCM16 化。
  用途：`tests/subtitle_live.rs`（英語音声の途中に日本語が混在する場合の言語判定確認）。

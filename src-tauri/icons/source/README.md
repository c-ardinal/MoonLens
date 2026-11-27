# アイコン原図

原図は生成スクリプト `gen.py`（ドット絵。ベクター原図なし）。

```bash
python3 src-tauri/icons/source/gen.py   # 要 Pillow・NumPy。src-tauri/icons/ へ出力
```

| 格子 | 用途 | 構成 |
| --- | --- | --- |
| 64 | 64px以上（整数倍で拡大） | 外環・ルーン帯・内環・月・四隅括弧 |
| 48 | 48px | ルーン帯を点列へ簡略化 |
| 32 | 32px | 点列 8 個 |
| 24 | 24px | 二重環・月・四隅括弧 |
| 16 | 16px | 環・月・四隅括弧 |

- 拡大は整数倍のみ（補間なし）。任意寸法への縮小・拡大は不可。寸法追加時は `LAYOUT` へ格子を追加。
- Windows / Linux 用（png・ico）は余白なし。macOS 用（icns）は図案 75 %、周囲は余白。
- 配色は本体の基準配色（`src/lib/pixel/palette.svelte.ts`）と共通。

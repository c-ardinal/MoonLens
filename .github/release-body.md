MoonLens は画面上の文字を端末内で OCR・翻訳する常駐ツールです。
翻訳・OCR は全処理をローカルで実行し、画像・テキストの外部送信はありません。

## 導入
| OS | ファイル |
| --- | --- |
| Windows | `MoonLens_*_x64-portable.zip`（任意の場所へ展開し `MoonLens.exe` を実行。インストール不要） |
| macOS（Apple シリコン） | `MoonLens_*_aarch64.dmg` |
| Linux | `*.AppImage`（実行権限の付与後に起動）、`*.deb`、`*.rpm` |

初回起動後、ホーム画面の「モデル準備」で翻訳モデル・OCR モデルを取得してください
（既定の翻訳モデル PLaMo 3 Translate 2B は約 1.8 GB）。通信の発生は取得時のみです。

配布物は未署名です。
- Windows：SmartScreen 表示時は「詳細情報 → 実行」で起動。
- macOS：初回起動の拒否後、「システム設定 → プライバシーとセキュリティ」の「このまま開く」で起動。
  「壊れている」表示の場合は `xattr -dr com.apple.quarantine /Applications/MoonLens.app` を実行。

旧版のインストーラー（`setup.exe`／`.msi`）で導入済みの場合は、アンインストール後に zip 版へ移行してください
（「アプリデータ削除」を選択しない限り、設定・履歴・モデルは継続利用可能）。

# 動画フィクスチャ

`tests/file_subtitle.rs` 用の短時間の動画。音声は `../audio/jfk.wav`（パブリックドメイン、
`../audio/README.md` 参照）、映像は生成画像。

| ファイル | 映像 | 音声 |
| --- | --- | --- |
| `jfk-1080p.mp4` | H.264 1920×1080 | AAC 44.1 kHz ステレオ |
| `jfk.mkv` | H.264 640×360 | MP3 48 kHz モノラル |
| `jfk-opus.webm` | VP8 320×240 | Opus（本体は非対応） |
| `silent-video.mp4` | H.264 320×240 | 無 |

## 再生成

```bash
J=../audio/jfk.wav
ffmpeg -f lavfi -i "color=black:s=1920x1080:r=5" -i $J -shortest -c:v libx264 -preset veryfast -crf 40 -pix_fmt yuv420p -c:a aac -b:a 48k -ac 2 -ar 44100 jfk-1080p.mp4
ffmpeg -f lavfi -i "color=black:s=640x360:r=5" -i $J -shortest -c:v libx264 -crf 45 -c:a libmp3lame -b:a 48k -ar 48000 jfk.mkv
ffmpeg -f lavfi -i "color=black:s=320x240:r=5" -i $J -shortest -c:v libvpx -b:v 20k -c:a libopus -b:a 24k jfk-opus.webm
ffmpeg -f lavfi -i "color=black:s=320x240:r=5" -t 2 -c:v libx264 -crf 45 -an silent-video.mp4
```

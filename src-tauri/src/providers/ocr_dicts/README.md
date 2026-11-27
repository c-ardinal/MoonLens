# Recognition dictionaries

One character per line, in the class order of the recognition model
(class 0 is the CTC blank, the class after the last line a space).

Each file is the `PostProcess.character_dict` list of the
`inference.yml` published beside the model, at the commit the model is
pinned to in `ocr_ppocr.rs` / `ocr_scripts.rs` (PaddlePaddle on Hugging
Face, Apache-2.0):

| File | Repository |
| --- | --- |
| `ppocrv6.txt` | `PP-OCRv6_small_rec_onnx` |
| `korean.txt` | `korean_PP-OCRv5_mobile_rec_onnx` |
| `cyrillic.txt` | `cyrillic_PP-OCRv5_mobile_rec_onnx` |
| `arabic.txt` | `arabic_PP-OCRv5_mobile_rec_onnx` |
| `thai.txt` | `th_PP-OCRv5_mobile_rec_onnx` |
| `devanagari.txt` | `devanagari_PP-OCRv5_mobile_rec_onnx` |
| `tamil.txt` | `ta_PP-OCRv5_mobile_rec_onnx` |
| `telugu.txt` | `te_PP-OCRv5_mobile_rec_onnx` |

The files must keep `\n` line ends and every line as it is: some lines
hold a character that looks like blank space.

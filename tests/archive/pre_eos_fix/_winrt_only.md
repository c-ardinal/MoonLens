# Lens pipeline evaluation

Detector: **winrt-only**

## Quality (residual fragments after inpainting)

| Input | Lines | Residual chars | Residual texts |
|-------|------:|---------------:|----------------|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 31 | 0 | (none) |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 18 | 0 | (none) |
| english0001.png | 8 | 0 | (none) |
| english0002.png | 9 | 0 | (none) |
| english0003.png | 26 | 3 | `:ID` |
| english0004.png | 7 | 0 | (none) |

## Timings (ms)

| Input | Preproc | WinRT | ocrs | Cards | Inpaint | Post-OCR | Total |
|-------|--------:|------:|-----:|------:|--------:|---------:|------:|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 222 | 274 | 0 | 235.2 | 11 | 40 | 834 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 325 | 377 | 0 | 176.7 | 41 | 74 | 1083 |
| english0001.png | 33 | 189 | 0 | 12.2 | 4 | 50 | 306 |
| english0002.png | 50 | 311 | 0 | 15.2 | 8 | 47 | 449 |
| english0003.png | 128 | 802 | 0 | 236.1 | 19 | 54 | 1284 |
| english0004.png | 40 | 188 | 0 | 9.2 | 5 | 71 | 331 |

## Aggregate (mean across 6 images)

- Residual chars: **0.5** / image
- Preprocess: 133 ms
- WinRT OCR: 357 ms
- ocrs detect: 0 ms
- Inpaint: 15 ms
- Post-OCR residual check: 56 ms
- **Total: 715 ms** / image

6 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.

# Lens pipeline evaluation

## Quality (residual fragments after inpainting)

| Input | Lines | Residual chars | Residual texts |
|-------|------:|---------------:|----------------|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 31 | 0 | (none) |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 19 | 0 | (none) |
| english0001.png | 8 | 0 | (none) |
| english0002.png | 9 | 0 | (none) |
| english0003.png | 26 | 3 | `:ID` |
| english0004.png | 7 | 0 | (none) |

## Timings (ms)

| Input | Preproc | WinRT | ocrs | Cards | Inpaint | Post-OCR | Total |
|-------|--------:|------:|-----:|------:|--------:|---------:|------:|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 225 | 311 | 322 | 290.8 | 12 | 43 | 1259 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 322 | 398 | 427 | 237.0 | 24 | 52 | 1539 |
| english0001.png | 29 | 170 | 221 | 30.5 | 4 | 44 | 514 |
| english0002.png | 44 | 235 | 259 | 59.9 | 6 | 53 | 693 |
| english0003.png | 130 | 636 | 260 | 216.1 | 13 | 50 | 1344 |
| english0004.png | 37 | 160 | 196 | 13.7 | 4 | 51 | 477 |

## Aggregate (mean across 6 images)

- Residual chars: **0.5** / image
- Preprocess: 131 ms
- WinRT OCR: 318 ms
- ocrs detect: 281 ms
- Inpaint: 11 ms
- Post-OCR residual check: 49 ms
- **Total: 971 ms** / image

6 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.

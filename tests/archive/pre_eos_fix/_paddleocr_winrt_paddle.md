# Lens pipeline evaluation

Detector: **winrt+paddle**

## Quality (residual fragments after inpainting)

| Input | Lines | Residual chars | Residual texts |
|-------|------:|---------------:|----------------|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 31 | 0 | (none) |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 20 | 0 | (none) |
| english0001.png | 8 | 0 | (none) |
| english0002.png | 10 | 0 | (none) |
| english0003.png | 26 | 3 | `:ID` |
| english0004.png | 7 | 0 | (none) |

## Timings (ms)

| Input | Preproc | WinRT | ocrs | Cards | Inpaint | Post-OCR | Total |
|-------|--------:|------:|-----:|------:|--------:|---------:|------:|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 256 | 268 | 5174 | 269.2 | 14 | 43 | 6078 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 332 | 426 | 3755 | 219.9 | 29 | 32 | 4886 |
| english0001.png | 34 | 182 | 1608 | 9.7 | 3 | 46 | 1898 |
| english0002.png | 60 | 304 | 1833 | 16.9 | 6 | 49 | 2288 |
| english0003.png | 134 | 656 | 3456 | 258.5 | 13 | 51 | 4610 |
| english0004.png | 37 | 177 | 1879 | 12.6 | 6 | 59 | 2189 |

## Aggregate (mean across 6 images)

- Residual chars: **0.5** / image
- Preprocess: 142 ms
- WinRT OCR: 336 ms
- ocrs detect: 2951 ms
- Inpaint: 12 ms
- Post-OCR residual check: 47 ms
- **Total: 3658 ms** / image

6 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.

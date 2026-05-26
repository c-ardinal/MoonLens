# Translate Quality Eval (routed)

**Samples per fixture**: 1

**Hardware**: Gpu

**Fixtures evaluated**: 2

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Compact | 1.00 | 1.00 | 1.06 | 100% | 148.7 | 2/2 |
| Standard | 1.00 | 1.00 | 0.88 | 100% | 126.6 | 2/2 |
| Quality | 1.00 | 1.00 | 0.87 | 100% | 128.3 | 2/2 |

## Per-UserTier × fixture detail

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 0.97 | yes | `Leméthodededécodagespeculativecombineunepetitemodelleurd'écritureetunegrandemodè` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 1.00 | 1.00 (0/0) | 1.15 | yes | `Décodagespeculativeassocieunmodéleprincipalplusgrandàunmodèlededéchiffrementplus` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.90 | yes | `Themethodofspeculativedecodingconsistsincombiningasmalldraftermodelandalargemain` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e2b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.87 | yes | `Speculativedecodingpairsasmalldrafterwithalargermainmodel:thedrafterpredictscand` |

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| debug-tech-en-to-fr-1 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.88 | yes | `Themethodofspeculativedecodinginvolvescombiningasmalldraftermodelandalargemainmo` |
| debug-tech-en-to-fr-2 | debug-pivot | `Direct:gemma-4-e4b-it-q4_k_m` | 1.00 | 1.00 (0/0) | 0.87 | yes | `Speculativedecodingpairsasmalldrafterwithalargermainmodel:thedrafterpredictscand` |

## Fixture catalogue

| Id | Category | Direction | Length (chars) |
|---|---|---|---:|
| debug-tech-en-to-fr-1 | debug-pivot | en → fr | 96 |
| debug-tech-en-to-fr-2 | debug-pivot | en → fr | 124 |


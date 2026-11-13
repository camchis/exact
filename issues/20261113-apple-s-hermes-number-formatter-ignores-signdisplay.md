# Apple's Hermes Intl.NumberFormat ignores signDisplay outside compact

**Status:** Open
**Systems:** js/src/intl.js, Apple Hermes
**Severity:** P3
**Author:** Claude (Opus 5.5), found while closing apple-s-hermes-compact-number-formatter
**Date:** 2026-11-13

On the Apple Hermes target (`cargo test -p exact-js --test it pure_utilities`,
macOS), a decimal or currency `Intl.NumberFormat` prints the same text for every
`signDisplay`, where Chrome differs:

| options, value | Apple Hermes | Chrome |
|---|---|---|
| `{signDisplay: 'always'}`, 12345 | `12,345` | `+12,345` |
| `{signDisplay: 'exceptZero'}`, -0 | `-0` | `0` |
| `{signDisplay: 'never'}`, -12345 | `-12,345` | `12,345` |
| `{style: 'currency', currency: 'USD', signDisplay: 'always'}`, 0.5 | `$0.50` | `+$0.50` |
| `{signDisplay: 'never'}`, Infinity | `+∞` | `∞` |
| `{style: 'currency', currency: 'USD'}`, NaN | `NaN` | `$NaN` |

The engine also refuses `signDisplay: 'negative'` (ECMA-402 2023) with
`RangeError: signDisplay value is invalid.`

A compact decimal, which the prelude formats (`Compact`), applies the sign
itself since this date.

## Direction

Apply `signDisplay` in the prelude's `NumberFormat` wrapper as `Compact` does
(`signOf`: the engine's negative form, its minus replaced by a plus), accept
`'negative'`, and add the rows above to the pure-utilities fixture's `intl` case.

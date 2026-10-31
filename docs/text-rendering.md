# Text rendering

Yute treats terminal text width as a runtime capability, not a universal constant. This matters for Indic combining marks, CJK, emoji ZWJ sequences and other grapheme clusters whose terminal-cell width may differ from the width predicted by a Rust Unicode table.

## Probe and width model

`yute text-probe` is a diagnostic command. `yute doctor` also includes its summary.

The probe writes bounded sample rows and uses terminal cursor-position reports (CPR, CSI 6 n) with short timeouts to compare real cursor advance with Yute's width model. It also queries DEC mode 2027 with DECRQM when the terminal supports the query. Capability probing starts after first paint; normal startup never waits for a terminal reply.

Probe samples cover Devanagari, Cyrillic, Greek, Japanese, Korean, Chinese, Thai, Tamil, Bengali, Arabic, Hebrew, combining accents and complex emoji. Results are cached for the terminal identity/version where possible.

Yute owns display width, wrapping, truncation and ellipsis in one TUI text module. UI modules do not call `unicode-width` directly. When the terminal agrees with grapheme-cluster widths, Yute uses the native cluster model. Otherwise it falls back to the safer measured model for that terminal.

## Complex-script fallback

Ratatui stores a terminal screen as cells. A shaped grapheme can occupy a different number of visible cells in the terminal than Ratatui reserved. When that happens, a normal buffer diff can shift following text or leave style holes.

For risky lines Yute can use an explicit-position layer: grapheme clusters are emitted at known cursor positions inside synchronized terminal output, while the corresponding Ratatui cells are excluded from conflicting diff output. Selection backgrounds are painted over the complete title rectangle before text, so continuation cells cannot leave holes in a selected Hindi/CJK/emoji title.

`[text].scripts` controls fallback behavior:

```toml
[text]
scripts = "auto" # auto | native | transliterate
emoji = "auto"   # auto | keep | strip
```

- `auto`: native text for scripts whose measured terminal behavior passes; transliterate only measured failures.
- `native`: always keep the original text.
- `transliterate`: prefer the approximate transliterated form where available.

`T` toggles original/transliterated text for the selected item. Detail views still prefer native text first.

Transliteration is a fallback, not translation. It is especially approximate for CJK/kanji and can lose semantic information.

## Bidirectional text

Arabic and Hebrew are best-effort. Yute can reorder bidirectional runs when the terminal does not provide suitable bidi behavior, but terminal font shaping, ligatures and script-specific presentation remain terminal/font responsibilities. Do not expect a terminal TUI to match a browser text engine exactly.

## Font coverage

Font checks run only under diagnostics, never in the render loop. `yute doctor` checks fontconfig coverage for Hindi, Japanese, Korean, Chinese, Arabic, Thai, Tamil, Bengali and emoji and reports missing coverage without changing system fonts.

Common Arch/CachyOS packages are:

```text
noto-fonts
noto-fonts-cjk
noto-fonts-emoji
```

On the development machine, Foot 1.28.0 reported grapheme support and DEC mode 2027 documentation. Native CPR measurements found width mismatches in Hindi matra samples, Tamil and Bengali, while fontconfig reported missing Korean/Chinese coverage. Those are machine-specific observations, not assumptions about other terminals.

## Verification

Automated tests use real strings including:

```text
रजनीकांत को पता चल गया इंडिया में 20 लाख करोड़ का काला धन है
Привет, мир — как дела?
日本語のタイトル ひらがな カタカナ
안녕하세요
```

Tests assert that wrapping does not drop graphemes, ellipsis width is bounded, borders stay intact and selected title rectangles have no unpainted cells.

Manual QA should run `yute text-probe` and inspect Home, Search and Watch in the actual terminal/font combination. Missing glyph boxes are a font-coverage problem when the probe's cell widths are otherwise correct.

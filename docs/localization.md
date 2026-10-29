# Languages and currencies

ShelfLedger bundles its translations, currency data and fonts locally. Language
switching and money entry work without a translation service, CDN or network
connection. Change them in **Settings → Preferences**. Preferences are saved per
library; changing the default currency never converts existing records.

## Initial language set

The initial selection uses the ten most widely spoken languages by total speakers
in the [2026 Ethnologue-based ranking published by Visual Capitalist](https://www.visualcapitalist.com/ranked-the-worlds-most-spoken-languages-in-2026/).
Romanian remains available in addition to those ten. Mandarin is represented by
Simplified Chinese; Portuguese uses the generic `pt` locale.

| Language | Native label | Code | Direction |
| --- | --- | --- | --- |
| English | English | `en` | Left to right |
| Mandarin Chinese, simplified | 中文（简体） | `zh` / `zh-Hans` | Left to right |
| Hindi | हिन्दी | `hi` | Left to right |
| Spanish | Español | `es` | Left to right |
| Standard Arabic | العربية | `ar` | Right to left |
| French | Français | `fr` | Left to right |
| Bengali | বাংলা | `bn` | Left to right |
| Portuguese | Português | `pt` | Left to right |
| Indonesian | Bahasa Indonesia | `id` | Left to right |
| Urdu | اردو | `ur` | Right to left |
| Romanian | Română | `ro` | Left to right |

The interface includes catalog and copy forms, reading, lending, finance,
imports, settings, backup confirmations, scanner guidance and accessible labels.
Arabic and Urdu set the document direction and Radix direction context; logical
spacing mirrors navigation, tables, popovers and dialogs. Native-script Noto
fonts are bundled for Arabic/Urdu, Devanagari, Bengali and Simplified Chinese.
Names, titles, descriptions, reviews, paths, ISBNs, imported column headers and
provider data retain their original content. Technical diagnostics from Rust,
the OS or third-party providers may remain in their original language. Dates
keep the project's DD.MM.YYYY policy; monetary separators follow the selected
language. Translations have automated completeness and browser checks, but have
not received independent native-speaker editorial review.

## Currency coverage and precision

The bundled [official SIX ISO 4217 List One](data/iso-4217-list-one.xml), published
17.09.2026, contains **165 distinct currencies and fund units with a defined
minor-unit scale**. All 165 appear in currency selectors, including CLF/UYW
(four decimals), dinars with three decimals, and zero-decimal currencies.
Selectors show ISO codes and localized names, with the official name as fallback
when the runtime does not recognize a newer code. Type a code while a selector
is open to jump to it.

[SIX is the ISO 4217 maintenance agency](https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html).
Its entries marked `N.A.` include metals, accounting/testing markers and `XXX`;
they do not define a monetary minor unit, so ShelfLedger does not invent one or
offer them as new currencies. Eight previously accepted codes retain their old
scales for compatibility: ANG, BGN, CUC, HRK, SLL, ZWL, XDR and XSU. XDR/XSU are
legacy application conventions, not ISO-defined decimal scales. Existing
records using a compatibility code remain readable and editable.

Amounts stay exact integer minor units with their currency code, up to
9,007,199,254,740,991 minor units. No cross-currency sums, exchange rates or cash
rounding are introduced. Decimal entry also accepts Arabic-Indic, Eastern
Arabic-Indic, Devanagari, Bengali and full-width digits, and Arabic decimal and
group separators. This does not change stored identifiers or date formats.

### Upgrading older libraries

The former currency table incorrectly treated AFN, ALL, COP, HUF, IDR, IRR, IQD,
KPW, LAK, LBP, MGA, MMK, PKR, SOS, SYP and YER as zero-decimal currencies.
Migration `0005_iso_currency_scales.sql` rescales their existing stored amounts
so the major-unit amounts previously displayed and entered stay the same.
For example, an old UI-entered 12 PKR becomes 1,200 minor units and still displays
12.00 PKR. This applies to acquisition price/shipping/tax, valuations, listings,
disposal price/fee/shipping and wishlist limits. Nulls remain null. The migration
runs once and refuses unsafe multiplication atomically rather than rounding.

New JSON exports use **formatVersion 4**, including edition cover images alongside
the checked marketplace comparisons introduced in format 3.
Format 1 snapshots remain importable
and receive the same scale correction transactionally. Format 2/3/4 snapshots are
already in ISO units and are not rescaled. Old ZIP backups run database migrations
before being restored. Do not label newly authored ISO-scale JSON as format 1.

## Adding another language

1. Copy `src/lib/locales/en.json` to a new language JSON file and translate every
   value. Keep keys and `{value}` placeholders unchanged. Keep proper names,
   codes and interchange column identifiers intact.
2. Import the bundle and add it to `dictionaries` in `src/lib/i18n.ts`. The
   `Language` TypeScript union derives from these keys.
3. Add its native label, BCP 47 locale and direction to
   `src/lib/locales/languages.json`. The UI and Rust preference validator share
   this registry. Include script fonts locally if the current set cannot render it.
4. Add any new product strings to **every** bundle. `aliases.json` connects older
   symbolic IDs to canonical English keys. Dynamic messages use `{value}` with
   literal interpolation; user content is never evaluated as translation syntax.
5. Run `pnpm test:web`, `pnpm test:rust`, `pnpm lint` and
   `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
   Check forms, dialogs, keyboard selection and mobile navigation in the new
   language. Have a native speaker review the text before claiming editorial approval.

The tests compare every bundle's keys and placeholders, check literal translation
calls and domain labels, and verify dynamic paths/values remain intact. Add the
new code to the intentional initial-language-set assertion when extending the set.
Unknown saved language preferences fall back to English.

## Updating currencies

Download List One XML from the SIX source linked above, review its publication
date and changes, and replace `docs/data/iso-4217-list-one.xml`. Then run:

```sh
python3 scripts/update-currencies.py docs/data/iso-4217-list-one.xml
pnpm test:web
pnpm test:rust
```

The script does not access the network. It updates the current list and retains
previously recognized codes. It stops if an existing decimal scale changes;
write and test an explicit migration and interchange-version upgrade first.
Rust and TypeScript both consume `src/lib/currencies.json`. Tests independently
compare it against every fixed-scale entry in the bundled official XML and
round-trip every supported code at the safe-integer limit.

## Verification record — 27.10.2026

- 290 frontend tests and 99 Rust tests pass; strict TypeScript/production build,
  `cargo fmt --check`, and Clippy with warnings denied pass.
- Browser checks selected all eleven languages and counted all 165 current
  currency options. Chinese and Arabic language preferences, plus a CLF currency
  preference, persisted after reload. Add Book uses that preference on first open;
  a regression test verifies delayed preference loading and exact four-decimal saving.
- Arabic was checked at desktop and narrow mobile widths, including navigation,
  document direction and horizontal overflow. Chinese/Arabic captures are in
  `docs/images/localization-*.jpg` and displayed in the README.
- Automated checks verify bundle key/placeholder completeness, every current ISO
  decimal scale, every supported currency at the safe-integer limit, transactional
  old-data upgrades, and version-one/version-two/version-three snapshot compatibility.

The production build reports a chunk-size advisory for the main bundle (about
606 kB before gzip). Native desktop rendering and independent native-speaker
editorial review of the expanded language set remain release checks.

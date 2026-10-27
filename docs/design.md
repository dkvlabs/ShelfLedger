# Design

Status: cinematic library redesign implemented 27.10.2026.

The interface takes its direction from a quiet, warmly lit private library:
ink-dark surfaces, restrained copper accents, warm ivory text, generous space
and large literary typography. It starts with the collection rather than a
statistics screen. Real saved records drive every count; empty states remain
honest and actionable.

## Visual system

Tokens live in `src/styles/globals.css`, with paired light/dark surfaces and
contrasting focus/destructive colors. The primary dark palette is approximately
ink #131514, copper #bd8963 and ivory #eee7dc. Cormorant Garamond provides display
type; Manrope provides controls and body text. Fonts are locally bundled through
Fontsource with Latin/Latin Extended support. System theme subscribes to OS
changes and removes its listener when preferences change.

A 224px desktop sidebar anchors navigation. The library pairs a large title with
Scan ISBN/Add book actions, warm photographic atmosphere, search/filter/sort,
and grid/table views. Book tiles use typographic treatments of actual saved
metadata, not fabricated covers. Smaller screens use a collapsible navigation
menu, wrapping controls and scrollable tables. Short desktop windows reduce
banner height. Motion is limited to brief entrance/hover transitions and respects
`prefers-reduced-motion`.

## Workflows and accessibility

- Book details expose work, edition and copy editors, tags and archived history.
- Inventory actions show the title and edition/copy identity; archives confirm
  and offer a visible restoration action. Financial disposal has an explicit dialog.
- Reading session editors expose want-to-read, reading, finished, paused and DNF,
  along with editable/clearable dates, ratings and reviews.
- Imports separate file choice, column mapping, preview, duplicate resolution
  and confirmation. No import writes happen at preview or cancellation.
- Inputs have labels, icons have accessible names, and dialogs retain Radix focus
  management. Mobile navigation is removed from focus order when closed.
- EN/RO product dictionaries cover screens, controls and state labels. User data,
  provider data and raw diagnostic errors are preserved as supplied.
- Dates display DD.MM.YYYY; money uses the record's currency-specific minor units.

Components live under `src/components/ui`, features under `src/features/*`, and
formatting/localization under `src/lib`. Query invalidation refreshes relevant
screens after writes and all screens after profile/restore changes.

## Artwork provenance

`public/images/library-atmosphere.png` is original generated decorative artwork,
created with OpenAI image generation for this redesign. It contains anonymous,
unlabelled books, not purported user inventory or fetched metadata. Prompt:
“Wide cinematic photographic still life of antique anonymous books on a wooden
desk beside a window, warm late-afternoon light, deep ink shadows, restrained
copper and ivory palette, no text, no logos, no recognizable book titles; the
right side falls into quiet darkness.” The library banner is decorative (`alt=""`).

The generated full-screen concept informed hierarchy, palette, copy and imagery;
implementation is validated against it with desktop/mobile screenshots. No award
or independent accessibility certification is claimed.

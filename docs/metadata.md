# ISBN and Romanian book metadata

A valid ISBN does not guarantee that a provider has catalogued its edition.
For example, ISBN **9786067882629** is checksum-valid and identifies
*Mitologie nordica* by Neil Gaiman on
[Libris](https://www.libris.ro/carte/mitologie-nordica-neil-gaiman/1258850).
On 29.10.2026 Open Library returned 404 and Google Books returned HTTP 429.
The alternative [ISBN Search page](https://isbnsearch.org/isbn/9786067882629)
returned an exact ISBN match. Its spelling is `Mitologie Nordiča`, publisher
Arthur, publication year 2017; it does not supply a page count or language.
ShelfLedger preserves the source data for review rather than silently guessing
corrections. The [publisher](https://www.editura-art.ro/info/carte/mitologie-nordica)
lists `Mitologie nordică`, 256 pages and January 2018. Catalog metadata can
conflict even for the same ISBN; review the physical edition before saving.

## Add the book

1. In **Add book**, type or scan the ISBN, then press **Look up**. Desktop/web
   tries Open Library → Google Books → ISBN Search, continuing after misses,
   quota errors or network failures. Related results with a different ISBN are
   rejected. A match stops further requests; there is no request while typing.
2. If the edition is missing, paste its Libris product-page URL into
   **Libris book link (optional)** and press **Import from Libris**. For the
   example above, use the link in this document. A typed ISBN must match the
   page's checksum-valid ISBN. You can also import a link with the ISBN blank.
3. Review the title, authors, publisher, publication date, language and pages,
   enter the physical copy details, then **Save book**. Importing metadata alone
   does not create a work, edition or copy, and it never imports a price.

You can always enter details manually. Typed ISBNs remain attached even when
lookup has no result or fails. Valid ISBNs do not show a checksum warning while
a lookup is loading. Results from an earlier input are ignored after you change
the ISBN/link or close the dialog.

## Cache and offline behavior

Successful lookups retain their provider and are cached in `metadata_cache`.
After importing the Libris link once, looking up the same ISBN uses that cached
metadata, including offline. Existing Open Library caches remain compatible.
Complete-library ZIP and JSON exports preserve caches for transfer to another
device. The source displayed in Add book reflects the actual provider.

The Android distribution has no INTERNET permission. It uses transferred caches
and manual entry only, including when desktop settings are restored. Setting
`metadata.lookup_enabled` to false blocks all online providers on desktop too;
already-cached results remain readable.

An uncached ISBN or Libris link with online providers disabled reports that lookup
is disabled on this device, rather than claiming the providers have no match.
On Android, fetch/import the metadata on desktop first and transfer a complete
library ZIP through **Settings → Export for another device**, then import it on
the phone. Scanning reads the ISBN; book details require cached metadata or manual
entry when offline.

## Provider limits and privacy

There is no background lookup, no request while typing, no universal ISBN
registry, and no promise that every Romanian ISBN is indexed. Google Books can
reject requests because of shared public API quotas (HTTP 429); that is reported
as provider unavailability rather than an invalid ISBN or a definitive no-match.
Libris support currently accepts its HTTPS `/carte/<slug>/<id>` pages, not
arbitrary retailers. If a provider changes its response or removes a book, use
manual entry; no unverified title-based match is substituted.

Libris import reads only Book JSON-LD from the requested page. It does not execute
scripts, load tracking resources or covers, or import reviews and sale prices.
ISBN Search is a bounded HTML fallback, not an official API contract. Only its
book-information block is parsed, with an exact checksum-valid ISBN-13 or
equivalent ISBN-10 required. Missing fields remain empty. A challenge or changed
page layout reports provider unavailability; the app does not bypass challenges.
HTTP 404 is a catalog miss. A failure from any provider is preserved when no
later source succeeds, so an outage is not presented as a definitive no-match.

Each automatic provider operation has a 12-second budget (at most 36 seconds
across all three); individual requests retain their 10-second timeout. Optional
Open Library author enrichment stops before that deadline, preserving the
already-matched edition and any author names resolved so far.
Requests have a 2 MiB body limit; cross-provider redirects and
arbitrary URL hosts are rejected. See [privacy policy](../PRIVACY.md).

## Verification

Deterministic Rust tests cover exact ISBN matching, Romanian Book JSON-LD,
three-provider fallback after failure/no match, timeout fallback, disabled providers, legacy/new caches,
publication dates and URL restrictions. Frontend tests cover source attribution,
typed ISBN retention, stale results, loading validation and Libris import/save.

The optional live regression makes a real request to the supplied Libris page:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_isbn_fallback_and_offline_cache -- --ignored
cargo test --manifest-path src-tauri/Cargo.toml live_user_supplied_libris_book -- --ignored
```

It is excluded from normal CI because provider availability and metadata can
change independently of the application.

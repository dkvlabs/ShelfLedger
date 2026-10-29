# ISBN and Romanian book metadata

A valid ISBN does not guarantee that a provider has catalogued its edition.
For example, ISBN **9786067882629** is checksum-valid and identifies
*Mitologie nordica* by Neil Gaiman on
[Libris](https://www.libris.ro/carte/mitologie-nordica-neil-gaiman/1258850).
On 29.10.2026 Open Library returned 404 for it; Libris's public ISBN search also
returned no results despite the product page existing. ShelfLedger does not
guess a title or substitute another edition to hide missing coverage.

## Add the book

1. In **Add book**, type or scan the ISBN, then press **Look up**. Desktop/web
   tries Open Library, then Google Books if the first provider misses or fails.
   Related search results with a different ISBN are rejected.
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
Requests have timeouts and a 2 MiB body limit; cross-provider redirects and
arbitrary URL hosts are rejected. See [privacy policy](../PRIVACY.md).

## Verification

Deterministic Rust tests cover exact ISBN matching, Romanian Book JSON-LD,
fallback after provider failure/no match, disabled providers, legacy/new caches,
publication dates and URL restrictions. Frontend tests cover source attribution,
typed ISBN retention, stale results, loading validation and Libris import/save.

The optional live regression makes a real request to the supplied Libris page:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_user_supplied_libris_book -- --ignored
```

It is excluded from normal CI because provider availability and metadata can
change independently of the application.

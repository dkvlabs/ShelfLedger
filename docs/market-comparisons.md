# Compare a book with OLX Romania offers

Open a book in **Library**, find the published edition under **Editions**, then
choose **Compare local prices**. Comparisons belong to that edition, keeping
translations, bindings and publications separate. Tracking-only books imported
from Goodreads can collect comparisons without creating a physical copy.

1. Choose **Search OLX by ISBN** or **Search OLX by title**. The desktop app opens
   your system browser; the web version requests a new browser tab. Check the
   ISBN, publisher, publication year, language, binding and condition yourself.
2. Paste the public listing URL, enter the advertised price and its currency,
   set the observation date, and add useful edition/condition notes. The default
   currency is RON because this workflow targets OLX Romania.
3. Check **I checked the edition, language and condition**, then **Save comparison**.
   Saving the same listing URL again updates that edition’s observation rather
   than counting it twice. Tracking parameters and fragments are removed.
4. Uncheck **Include in estimate** for bundles, different editions, poor matches,
   stale advertisements or outliers. Excluded observations remain saved. Use
   **Edit** to correct a listing and verify it again before saving.
5. Read the included count, range and median for each currency. Currencies are
   never combined or converted. With two middle prices, their midpoint is rounded
   down only when it falls between minor units, with an explanatory label.
6. Choose a currency under **Simulate an asking price**. **Use median** fills the
   asking price; adjust it and enter your selling fee and shipping cost to see
   estimated net proceeds. Blank costs mean zero; invalid costs prevent a result.
   Fees and shipping are your assumptions, not rates supplied by OLX.
7. To record a valuation, select an active physical copy of the same edition,
   choose your confidence, and press **Save median estimate**. This saves the
   included listings’ median in the selected currency, not the simulator amount.
   It retains URLs, observation dates, original minor-unit amounts and notes in
   the valuation’s evidence. Its own observation timestamp is the UTC save time.
   Expand **Saved estimate evidence** to inspect the latest OLX-based valuation.

Saving comparisons or valuations does not create an OLX advertisement, change
ownership, or record a sale. Advertised prices are not completed sale prices;
negotiation, stale offers, shipping and edition differences affect relevance.
No confidence is inferred from sample size. Choose it based on your assessment.

All saved comparisons, exclusions, calculations and valuations work offline.
Opening an OLX search or listing requires internet and shares the chosen search
terms/URL with OLX through your browser. ShelfLedger does not automatically fetch,
scrape, embed or monitor OLX pages. Only HTTPS OLX Romania public listings and
book-search links can be opened by this feature.

Comparisons are included in format 3 JSON exports and ZIP backups. Complete format
1/2 JSON exports remain importable with an empty comparison table. CSV exports
remain inventory reports; use JSON or ZIP to preserve comparison evidence.

Implementation: migration `0006_market_comparables.sql`, `domain/market.rs`,
`repositories/market.rs` and named commands shared by the desktop/web transports.
The UI strings ship in all eleven interface languages. Rust regressions exercise
validation, canonical URL updates, currency partitioning, exclusions, persistence,
copy ownership/edition constraints, immutable evidence and snapshot compatibility;
frontend tests cover verification, saving, exclusions and exact simulations.

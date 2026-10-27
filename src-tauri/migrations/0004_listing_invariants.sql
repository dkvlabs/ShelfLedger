-- Repair stale active listings from versions before the lifecycle fix.
UPDATE sale_listings SET status = CASE
  WHEN (SELECT ownership_status FROM copies WHERE id=copy_id)='sold' THEN 'sold'
  ELSE 'withdrawn' END
WHERE status='active' AND copy_id IN (
  SELECT id FROM copies WHERE ownership_status IN ('sold','donated','traded','lost')
);
UPDATE sale_listings SET status='withdrawn'
WHERE status='active' AND rowid NOT IN (
  SELECT max(rowid) FROM sale_listings WHERE status='active' GROUP BY copy_id
);
CREATE UNIQUE INDEX idx_listings_active_per_copy ON sale_listings(copy_id) WHERE status='active';

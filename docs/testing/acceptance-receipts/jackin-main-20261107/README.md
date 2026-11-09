# Jackin binary baseline acceptance receipts

This directory preserves the 57 workstation acceptance receipts for Jackin binary baselines already present in TCB main. Receipt JSON files are copied byte-for-byte from the approved rollout evidence. No acceptance command was run while preparing this evidence.

## Provenance

- Live TCB `main` verified through GitHub: `534e0ff5af75b840d93038dd02ece0839cf4f71b`.
- The approval transaction was performed against the isolated rollout source at `f539d9a61b1fe1e9fab13f4e976c872b09b8fb9a` using Tuiscotti `fec38841b10a593f64498f1e6daf2f4246a4d70f`.
- Each receipt records `Explicit user approval for all 57 selected Jackin candidate baselines.`
- `manifest.json` links each test ID to its active baseline SHA-256 and original receipt SHA-256. Every ID/hash pair is in the selected `jackin-79` rows of the approved selection manifest, whose SHA-256 is `88c130c80ad15dc6e5e66ccad6d51d0d82306005498a15f382239c3cde2b46f4`.
- All 57 active files under `apps/jackin/tests/functional/baselines/functional/` at `534e0ff5af75b840d93038dd02ece0839cf4f71b` were independently hashed and matched both the receipt candidate and baseline SHA-256 fields.

The deterministic manifest SHA-256 is `f41c5427b5d60c2f101754ffe32ff0c0d209d06c97b9761fba97c45df410f9b1`. This records provenance and byte identity; it does not grant new approval.

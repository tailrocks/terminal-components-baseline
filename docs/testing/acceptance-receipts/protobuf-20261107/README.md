# Protobuf baseline acceptance receipts

These 105 wrapper receipts preserve candidate hashes, fixture identity, comparator, acceptance expectation, and precondition for the accepted protobuf samples in `tests/functional/baselines/`. They remain separate from the original JSON receipt metadata in the parent directory and from the CLI result JSONs in the corrected transaction replay evidence.

The transaction sequence and per-sample source lineage are indexed in [binary-baselines-20261108.tsv](../../binary-baselines-20261108.tsv). The two owner-head refresh operations use their prior staged digests as `expect_baseline`; the other 103 operations use `missing`. `tablepro_query_syntax_error` is excluded.

# openentropy-server

HTTP server for OpenEntropy.

Provides API endpoints for entropy bytes and health reporting, including an
ANU-style `/api/v1/random` response format with explicit byte-length semantics.

For `/api/v1/random`, `length` is always output bytes. Responses include
`length` as the returned byte count and `value_count` as the number of encoded
items in `data`. `type=hex16` and `type=uint16` require an even byte length.
Invalid query parameters return JSON `400 Bad Request` responses.

`/pool/status` reports the aggregate healthy-source count as `sources_healthy`.
Per-source rows still use `healthy` as a boolean.
`/sources` and `/pool/status` source rows expose:
`name`, `healthy`, `bytes`, `entropy`, `min_entropy`, `autocorrelation`,
`time`, and `failures`.

## Install

```toml
[dependencies]
openentropy-server = "0.12"
```

## Repository

https://github.com/amenti-labs/openentropy

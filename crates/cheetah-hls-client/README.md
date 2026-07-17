# cheetah-hls-client

HLS / LL-HLS playlist and segment client.

## Responsibility

- Parse HLS master playlist and select variants by bandwidth.
- Future: segment download, playlist refresh, and LL-HLS blocking reload.

## Constraints

- Forbids `unsafe_code`.
- Depends on container, timeline, and types crates.
- Network operations require the `std` feature.

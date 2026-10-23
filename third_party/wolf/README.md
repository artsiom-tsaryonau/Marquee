# Wolf source contract

Upstream: https://github.com/games-on-whales/wolf

Reviewed revision: `2d2ae0021ad812f79a966dd10e8289e0fae98ecb` (2026-10-19),
selected from the active `beta` branch on 2026-10-23. Upstream's `stable` branch
still points at a 2024 revision without the current Unix control API. Pin this
commit when reproducing the integration; do not substitute a floating image tag.

`openapi.json` is an unmodified copy of
`docs/modules/dev/partials/spec.json` at that revision. `LICENSE` is the
unmodified upstream MIT license. The Rust adapter's request/response structures
are selectively adapted from:

- `src/moonlight-server/api/api.hpp`: envelopes, pairing, clients and stop requests.
- `src/moonlight-server/api/unix_socket_server.cpp`: routes and HTTP framing.
- `src/moonlight-server/api/endpoints.cpp`: acknowledgement/error semantics.
- `src/moonlight-server/events/reflectors.hpp`: app and session wire projections.

SHA-256 of the retained OpenAPI file:
`1e87843d21bb763522a913b029dd4ca5a6ab28070f2183026502309e9eb7726e`.
The retained schema and license were compared byte-for-byte with the pinned
source checkout.

In particular, the session reflector's `client_id` contains the **session ID**;
its `aes_key`, `aes_iv`, and runner/pipeline configuration must not enter public
Marquee diagnostics. Pending pair secrets remain internal to the adapter.
The retained schema contains field definitions, not live credentials.

Marquee implements its own bounded Rust Unix-socket transport. No Wolf binary,
compositor, GStreamer plugin, Moonlight decoder, or GPL implementation code is
vendored in this increment. Wolf's dependency/container/codec licenses need their
own inventory before distributing a managed backend. Retain this notice and the
MIT license when redistributing the adapted contract or schema.

Reproduce source inspection (outside the tracked source tree):

```sh
git clone --no-checkout https://github.com/games-on-whales/wolf.git data/streaming/sources/wolf
git -C data/streaming/sources/wolf checkout --detach 2d2ae0021ad812f79a966dd10e8289e0fae98ecb
```

This is a source checkout, not an installer or a validated streaming runtime.
See [the implementation gates](../../docs/STREAMING.md).

# Graphene Phase 0 Foundation API

This note documents concrete Phase 0 implementation behavior without changing the normative
architecture documents.

## Construction

Graphene is created from an explicit data root. Construction validates immutable configuration,
normalizes the current platform, initializes the data-root layout, and constructs one shared HTTP
client and one operation registry for that engine.

```rust,no_run
# use graphene::{Graphene, NetworkConfig};
# async fn example() -> Result<(), graphene::GrapheneError> {
let graphene = Graphene::builder("./graphene-data")
    .network(NetworkConfig::default())
    .build()
    .await?;
# let _ = graphene;
# Ok(())
# }
```

Multiple `Graphene` values may coexist with different roots. No global Graphene singleton, global
runtime, data-root cache, or tracing subscriber is installed.

Phase 0 defaults are explicit: 10-second connect timeout, 60-second request timeout, three attempts
per source with 100 ms initial/2 s maximum backoff, at most 10 redirects, system/default proxy
behavior, eight active downloads, and 256 queued events per subscription. Builder validation rejects
zero or out-of-range resource limits and malformed explicit proxy/user-agent configuration before
data-root initialization begins.

## Operation semantics

All long work uses `Created`, `Queued`, `Running`, `Cancelling`, `Succeeded`, `Failed`, and
`Cancelled`. Terminal states are retained and never transition back to non-terminal states.
Cancellation is idempotent, propagates from parent to child tokens, and is checked by download work
before requests, while waiting for concurrency, between streamed body chunks, during retry delay,
after transfer, and before cache commit. Cancellation and success share one state-lock ordering
point, while every parent/child cancellation tree shares one cancellation ordering lock. Immediately
before a verified whole-file cache rename, the operation seals cancellation as its documented point
of no return: cancellation that wins before the seal terminates as `Cancelled`; a late request after
the non-interruptible safe-commit boundary cannot relabel committed success as cancellation.

Operation subscriptions use bounded queues. Progress events may be coalesced under pressure. Current
state is retained independently of the event queue, and a terminal event is admitted at most once. A
late subscription receives the retained terminal event immediately; hosts can also query
`OperationHandle::snapshot()` at any time.

## Data-root layout and cache identity

Phase 0 initializes `.graphene-layout.json` with layout version `1` plus the top-level `config`,
`instances`, `shared`, `cache`, `database`, and `logs` hierarchy specified by the implementation
plan. Unknown files are preserved and initialization is idempotent.

Verified cache identity prefers SHA-256 and falls back to SHA-1:

```text
cache/objects/sha256/<first-two-hex>/<digest>
cache/objects/sha1/<first-two-hex>/<digest>
```

Incomplete transfers live only below:

```text
cache/downloads/temporary/*.part
```

The network layer streams to a Graphene-managed `create_new` temporary file and verifies expected
size plus every declared SHA-1/SHA-256 digest before the storage layer is asked to commit. Managed
cache roots and commit candidates are checked for lexical and canonical containment so symbolic-link
escapes are rejected at security-sensitive boundaries. Committed objects are never populated by
copying an incomplete stream into the final path. Replacement is serialized by a filesystem lock;
Unix uses atomic rename replacement, while Windows preserves the previous file in a backup until the
verified replacement rename succeeds.

## Artifact acquisition example

```rust,no_run
# use graphene::{Artifact, ArtifactIntegrity, ArtifactSource, Graphene, Sha256Digest};
# async fn example() -> Result<(), graphene::GrapheneError> {
# let graphene = Graphene::builder("./graphene-data").build().await?;
let sha256: Sha256Digest =
    "0fa051629d6f04851721b0a8d6002f23ed322719d0e81b9df49991d2cf7bf828"
        .parse()
        .expect("validated fixture hash");
let artifact = Artifact::new(
    vec![ArtifactSource::new("http://127.0.0.1:8080/fixture")],
    ArtifactIntegrity::none().with_sha256(sha256),
)
.with_expected_size(98_304);

let prepared = graphene.artifacts().acquire(artifact, None);
let operation = prepared.operation();
let events = operation.subscribe();
let verified = prepared.await_result().await?;

assert!(verified.path.is_file());
let _ = events;
# Ok(())
# }
```

Production transport keeps TLS certificate validation enabled. Tests use only local deterministic
HTTP fixture servers.

## Errors, diagnostics, and tracing

`GrapheneError` separates stable `ErrorCode`, broad `ErrorKind`, structured non-secret context,
developer text, and a private concrete source chain. Hosts must branch on codes/kinds rather than
parse messages. Diagnostics are separate structured evidence.

Graphene emits through the `tracing` facade only and never installs a subscriber. Network tracing
uses operation/artifact IDs, attempt number, and source host. Artifact source URLs and explicit
proxy configuration are redacted from `Debug`; credentials and sensitive query strings are never
added to error context or tracing fields.

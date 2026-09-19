# stuf

Supply chain security for Rust, designed for targets from bare-metal to cloud. 

**Docs/site:** https://juneswift.github.io/stuf-docs/

## Architecture

```text
stuf-core              # trust kernel: Verified<T>, Verifier<T>, no_std
stuf-encoding          # canonical serialization and decoding traits
stuf-env               # crypto, transport, storage, and clock bindings
stuf-protocols/tuf     # TUF verification and publishing logic
stuf-examples          # publisher and embedded toaster demos
````

## How it works

Applications compose verification profiles using stuf's primitives:

`stuf-core` defines the verification trait and the `Verified<T>` trust type, giving all protocols uniform type-level enforcement for verified data.

`stuf-encoding` owns canonical serialization and decoding. The TUF implementation uses RFC 8785 / JCS canonical JSON for signed metadata.

`stuf-env` provides pluggable platform bindings for crypto, transport, storage, and clocks.

`stuf-protocols/tuf` implements the first protocol profile: TUF v1.0.

## Embedded

`stuf` supports `no_std + alloc` and includes a no-heap TUF verifier profile for constrained targets.

## Examples

The workspace includes:

```text
publisher             # generates signed TUF demo metadata and target files
toaster               # ARM Cortex-M demo using the small-heap verifier profile
toaster-no-heap       # ARM Cortex-M demo using the no-heap verifier profile
```

## Status

Early. Core architecture is in place. The first protocol (TUF) is implemented. Two embedded "small heap" and "no-heap" verifier profiles are implemented. A smaller, maximally constrained bare metal profile is planned.

## License

Apache-2.0

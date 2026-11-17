# Primitive performance baseline

Run:

    cargo xtask bootstrap
    cargo run --release -p xtask -- bench

The tool checks pinned references, compiles the original JVM code, runs 20
warmup passes and nine measured passes per workload in each runtime, verifies
matching checksums, and saves raw samples and source fingerprints to
runs/bench/report.json.

The initial Windows x86_64 run used an AMD Ryzen 9 9950X3D, Rust 1.86 release
with thin LTO/codegen-units=1, and Temurin OpenJDK 25.0.3. Both workloads run
4,000,000 operations per pass:

| Workload | JVM median | Rust median | JVM/Rust time |
| --- | ---: | ---: | ---: |
| ISAAC enabled stream | 10.28 ms | 9.69 ms | 1.06 |
| Repeated Buffer.g4 reads | 3.27 ms | 2.11 ms | 1.55 |

These are isolated synthetic measurements on a concurrently used workstation,
without CPU affinity control. They include per-pass object setup and exclude
process startup. The small ISAAC difference should not be treated as a
significant application speedup; opcode ISAAC is disabled in the default
reference profile anyway.

This does not measure a game server, renderer, networking, content execution,
or model inference. It establishes neither a world speedup nor a player limit.
The [saved JSON](initial-windows-primitives.json) contains all samples, input details, matching checksums, build
versions, reference revisions, and native/oracle source hashes.

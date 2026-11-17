# Native cache loading

The Rust library reads the pinned sector cache and extracts original file bytes:
protocol 5/6 reference tables, sparse IDs/names, XTEA, and multi-file stripes.
Definition interpretation, script execution, and gameplay remain separate work.

## Verify the complete corpus

    cargo xtask bootstrap
    cargo xtask cache-fetch
    cargo run --release -p xtask -- cache-audit --jvm

Acquisition reads the locked Git LFS pointers, downloads missing objects through
HTTPS curl, and checks each size/SHA-256. Identical verified visual-cache files
can be reused. The complete cache lives in assets/cache.

The audit invokes complete unmodified Cache, BufferedFile, Js5Index and Js5
classes. It compares every extracted byte and fallback ID with Rust:

| Item | Count |
| --- | ---: |
| Pinned LFS files | 31 |
| Archives | 29 |
| Groups | 68,499 |
| Extracted files | 411,863 |
| Extracted bytes including fallbacks | 302,890,001 |
| Original client fallback groups | 72 |

Recorded evidence is parity/cache-audit.json; new reports go to runs/cache-audit.
Reports identify sources, assets, XTEA configuration, reference commits, and
tool versions. This is a cache-byte audit, not gameplay or throughput validation.

## Runtime API

Store::open accepts a directory, StorageMode, and Limits. Resident loads dat2
once; cloned stores share that memory. File reads carry explicit offsets and
validate group/part/archive headers. Reads never use a shared seek-then-read
pair. Windows seek_read updates its cursor but takes an explicit offset; see
the [Rust documentation](https://doc.rust-lang.org/std/os/windows/fs/trait.FileExt.html#tymethod.seek_read).

Archive::open loads an index; file/group requests take group/file IDs and an
XTEA key. All-zero keys disable decryption. Returned bytes have shared Arc
ownership for reuse across clients. Runtime loading invokes no external tools.

Concurrent loads of one group share a decode. Cache-map locking covers lookup
and insertion; disk reads, decryption, and decompression happen outside it.
Limits cap container/output sizes, IDs/file counts, retained entries and retained
buffer/control storage. When retention is full, reads continue uncached.
Allocator/map bookkeeping is outside the byte estimate and bounded separately
by entry/file limits. This is not a process-wide memory limit.

## Compatibility details

RT4 uses a fixed ten-byte GZip header and raw DEFLATE, ignoring trailer CRC/ISIZE.
Edited pinned assets depend on that behavior.

The pinned keys fail to decompress 72 map groups in both runtimes. Original Js5
substitutes byte [0] and then applies its usual extraction rules. For a one-file
group that byte is returned; a striped group can yield empty files.

Archive::open defaults to strict errors. Client parity requires
Archive::open_with_policy with FailurePolicy::Rt4Client.
GroupFiles::compatibility_fallback exposes the original failure. The audit uses
this policy and verifies fallback IDs as well as resulting bytes. Changing asset
or key data would change the behavioral baseline and requires review.

Reference metadata exposes group CRC/version fields. Local acquisition/audits
verify full assets against LFS hashes. Network-provider CRC/version handling
and disk writes remain pending. Convenience name lookup accepts ASCII; full
JagString encoding/case folding remains pending.

## Regression checks

    cargo test --workspace
    cargo xtask oracle --check
    cargo xtask cache-oracle --check

The committed portable cache corpus uses original Cache.write to create sector
files and original read/index/extraction classes for expectations. Tests cover
sparse IDs, name collisions/holes, signed stripe deltas, encryption, empty files,
corruption, concurrent reads, shared loads, and cache budgets.

Regular CI repeats those fixtures. Manual compatibility runs also acquire the
complete cache and repeat the full original-vs-native audit.

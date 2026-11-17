# Native revision-530 login codecs

The shared Rust protocol now writes the pinned RT4 login request and parses it
with the executed Kotlin server rules. The server endpoint performs that parsing
in its bounded compute pool. Configured endpoints then run native account checks
on separate bounded workers and return their rejection codes. Accepted accounts
run native player/communication metadata setup: parse failures receive
UnexpectedError (0), and prepared records receive LoginServerOffline (8) until
character ownership, profile loading and world streaming exist. See
[player-metadata.md](player-metadata.md). Cache-only mode skips account checks and
returns 8 for valid requests; parser failures retain their original response.
Native account rules are documented separately in [accounts.md](accounts.md).
What follows a successful decode in the world (Login.proceedWith, LoginParser,
Player.init, LoginConfiguration) is the library of
[world-login.md](world-login.md), which nothing installs yet.

## Runtime behavior

`protocol::login::LoginRequest` encodes normal/reconnect packets with the original
UID/settings layout, 28 archive CRCs plus the optional fake index-28 zero, and an
RSA-wrapped verification byte, four seeds, Base37 name, password and optional
three empty extra strings. `RsaKey` follows signed Java BigInteger input/output,
including leading zero bytes. Constants match the publicly checked-in reference
keys. Configured keys can be supplied through the Rust API.

`decode` returns the original response and optional partial `LoginInfo`, with an
explicit callback for online-name lookup. It preserves signed byte/short fields,
wrapping seed-plus-50 arithmetic, validation order and the distinction between
outer parsing errors and the RSA framing-underflow sentinel. The original code
reads the CRC values but does not verify them here. Additional RSA strings are
sent by the client and ignored by this server parser.

Compatibility details follow executed source rather than configuration names or
stale comments:

- `LOGIN_USE_STRINGS=true` does not change the actual Base37 username field.
- `ByteBufferUtils.getString` sign-extends bytes above 127 to U+FF80 through
  U+FFFF. Rust preserves that conversion for the server's decoded password.
- Default server ISAAC getters return zero. Incoming seeds and outgoing seeds
  plus 50 are still decoded and checked against the original cipher state.
- The server reply switch appends a server-key byte for ordinal 21, which is
  ErrorLoadingProfile in the current enum; its comment calls that MovingWorld.
  Rust preserves the executed ordinal. The RT4 client reads a following detail
  byte for codes 21 and 29.

`net::login::LoginPeer` handles native TCP/binary WebSocket login transport,
fragmented handshakes, nonce insertion, reply fields and detail bytes. It leaves
coalesced game bytes available through `into_transport`. Receiving success
metadata alone is not a completed world login: the original client also waits
for the initial rebuild packet. Retry/UI states, game packet decoding and
browser network bindings remain pending.

## Differential checks

Ordinary tests use portable committed expectations and require no JVM, compiler
or game assets:

```powershell
cargo test -p xtask --test login
cargo test -p rs09-server --test login
```

To reproduce the original methods, install Python 3.10+ and JDK 11:

```powershell
cargo xtask bootstrap
cargo xtask login-oracle --check --java-home PATH_TO_JDK_11
cargo xtask login-interop --java-home PATH_TO_JDK_11
```

If JDK 11 is already on PATH, omit `--java-home`. `RS09_ORACLE_JAVA_HOME` is also
supported. The original Kotlin 1.8.20 compiler cannot parse the installed JDK 25
version string; the oracle therefore uses a separate JDK 11. Neither system Java
nor reference checkouts are modified. `--python PATH` or `RS09_PYTHON` selects an
explicit Python executable when the Windows alias is unavailable.

The generator acquires pinned compiler jars from Maven Central over HTTPS,
verifies size/SHA-256 against `oracle/login/kotlin.lock.json`, and builds under
the ignored `runs/` folder. The compiler version matches the server POM. Java,
Kotlin, Python and compiler jars are test tooling only; runtime login uses Rust.

The 2,049 cases cover original client packet bytes, signed RSA, Base37 names,
every nonzero byte's server text conversion, metadata, normal/reconnect flags,
optional fields, wrapped seeds, truncated packets, bad revisions/opcodes,
RSA verification/underflow, lookup order, reply codes, rights and index truncation.
Fixtures/provenance live under `fixtures/login/`. `--check` refuses any output or
source-hash change. Intentional regeneration requires reviewing both files.

`oracle/login/generate.py` extracts the packet-writing slice from LoginManager
verbatim, removing only local deobfuscation annotations. It also copies selected
Kotlin Login methods and the original LoginWriteEvent methods verbatim. Complete
LoginInfo, AuthResponse, ISAACCipher, ISAACPair and ByteBufferUtils compile
unchanged. Test adapters supply byte storage, static client fields, logging,
online-name lookup, and player/session reply holders. JagString's Base37 method
and the server's long-name conversion are original methods. Other JagString
behavior is outside this adapter.

## Live interoperability and bounds

`login-interop` runs both directions over real TCP: 64 requests from the original
JVM writing slice into Rust, then eight native-peer sessions against original
JVM decoding/reply methods. It checks nonce seeds, reply fields and game-byte
retention. The JVM side is an isolated codec oracle with explicit adapters, not
the original game server. It uses the portable committed sector fixture for
native endpoint startup and requires no full cache download.

Reports under `runs/login-interop` record both directions, source hashes,
toolchain versions and shutdown accounting. The recorded milestone is
`parity/login-interop.json`. It is protocol evidence, not gameplay or a capacity
benchmark.

Native integration tests also replay the applicable login corpus over TCP and
WebSocket with fragmented packets. A 64-request RSA burst shares one compute
slot while cached I/O remains responsive. Login CPU work is admitted before
blocking dispatch; no Java process runs inside the server. Existing ingress,
connection, queue and timeout bounds still apply. Native encoding rejects NUL
strings and wire-unrepresentable lengths rather than reproducing unchecked
buffer writes.

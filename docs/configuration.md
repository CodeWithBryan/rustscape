# Native server configuration

`rs09-server::config::Configuration` loads the pinned server configuration format
without Java. It implements the 82 ordered ServerConfigParser assignments,
their initial and parsed defaults, world settings construction, paths,
locations and authentication-policy projection. `--config` now connects cache
paths, listener ports, retained settings and account-backend selection to the
native endpoint. The world/gameplay consumers remain pending.

`Configuration::load(Path)` reads a file using the reference's filename/path
conversion. `apply(text, separator, legacy_java_version)` supports deterministic
tests and ordered reloads. `constants()`, `world()` and `auth_settings()` expose
the parsed startup data. `data()` retains the complete native TOML tree, while
`boolean(key, default)` implements the content configuration access used for
personalized shops. Configuration contains credentials and deliberately does
not implement Debug.

## Preserved behavior

- Defaults before loading differ from parser fallbacks. For example, missing
  auth/persistence settings enable both and disable default administrator rights,
  whereas the initial constants and shipped default.conf use development mode.
- Assignments occur in source order. A failed load can leave changed constants
  and an earlier world-settings object. Successfully parsed TOML remains visible
  to content even when later settings validation fails; a syntax failure retains
  the previous TOML object.
- `@data` replacement uses the current DATA_PATH. Path conversion prefers forward
  slashes, removes empty components and appends a separator, even to filenames.
  Stored paths retain that behavior; the file-opening boundary accounts for
  java.io.File discarding the final separator. Relative paths use the process's
  working directory, rather than the configuration file's directory.
- Locations parse every comma token, ignore additional valid coordinates after
  the third, and reject whitespace/non-integers as the original does. x/y wrap
  through signed 15-bit storage; plane values normalize modulo four. Numeric
  strings use Java decimal parsing, including supported BMP Unicode digits.
- Long settings truncate to signed 32-bit values. MOTW substitutes `@name` once;
  log levels uppercase and fall back to VERBOSE.
- The toml4j string adapter preserves its replacement order, including escaped
  Windows paths, four-digit `\\U` handling and multiline continuations. Literal
  strings retain their literal interpretation. Attribution and the MIT notice
  are under THIRD_PARTY_NOTICES.md and third-party/.

The original derives JAVA_VERSION from its host JVM. Tests control that string,
including its substring quirks and failures. Native loading uses compatibility
value 11, matching the pinned POM target. This selects the reference's compact
JSON save path when persistence is integrated; it does not query or run Java or
a JavaScript engine. Player saving itself remains unimplemented.

Native errors are typed and do not terminate the process. The library records
which tested failures made the reference exit with code zero. JVM log text,
URL-loading overloads, the full toml4j grammar/extension surface, default-locale
Unicode casing outside the tested locale, and GameSettings' separate JSON parser
are not covered by this port. Source-file replacement status remains pending.

## Reproduce checks

```powershell
cargo test -p rs09-server --test config
cargo xtask config-oracle --check --java-home PATH_TO_JDK_11
```

The oracle currently runs on Windows with Python 3.10+ and JDK 11. `--python PATH`
and the existing oracle environment variables are supported. Test-only Kotlin,
TOML, Gson and JSON-simple artifacts are pinned by size/hash. It compiles the
complete unmodified original parser/constants, original GameSettings and Database
classes, and a verbatim ContentAPI.getServerConfig accessor. Explicit adapters
supply logging, a world holder, an intercepted JVM exit, unrelated teleport
initialization and Location construction using the original nested LocationHash.

The 742 cases compare all selected constants, world fields/string, retained TOML
presence, personalized-shop policy and original listener-port selection after each action, plus path/location
results. They cover the unchanged default.conf, missing/type-invalid fields,
integer boundaries, escape behavior and reload failures. Five Rust tests also
verify fixture provenance, native file loading and authentication projection.
Ordinary tests use committed fixtures and need no JVM. Neither configuration
loading nor these checks establish world gameplay or a player-capacity limit.

Startup/factory selection and the LoginReadEvent authentication branch are now
connected. Clock-aware account APIs preserve time sampling around storage and
password work. Character ownership, communication parsing, profile loading and
first-world rebuild remain the next integration boundaries. The complete native
game startup sequence is not established by these selected settings checks.

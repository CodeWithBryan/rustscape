# Native timer registration and factories

`server::timers::registry` now supplies the behavior of TimerRegistry and the
ContentAPI spawn/get-or-start wrappers, connected to the native timer manager.
Registration and lookup use JDK 11 ROOT lowercasing. Entity active-timer queries
remain case sensitive, as in the original. A lookup for a differently cased ID
can therefore construct another instance even when the entity holds a timer
whose own identifier differs only by case.

Registration logs before checking the identifier, retains the original prototype
handle, refuses duplicate normalized identifiers, and appends only accepted
automatic timers. Duplicate warnings, errors and partial failures preserve the
original order. Exact-key `put` models public-map insertion/replacement, including
null values; arbitrary map removal/reflection APIs are not part of this helper.

String lookup returns a fresh factory instance or an absent result. Type lookup
chooses the first matching prototype in original String HashMap iteration order,
constructs once, and checks the returned instance's type. It does not continue
searching after a mismatched result. Empty arguments select `retrieveInstance`;
nonempty arguments select `getTimer`. Typed spawn requires a non-null result;
identifier spawn permits absence. Get-or-start returns an existing entity timer
before invoking any factory, otherwise constructs and registers in source order.

Automatic installation visits the live shared list in registration order. It
checks the prototype's exact identifier on the entity, creates an instance, then
uses the nullable registration wrapper. Callback changes to the automatic list
obey the original ArrayList iterator checks, including prefix mutations left
behind after failure. Saved timer parsing can use `Registry::instance` through
`Context::resolve`, and is exercised with the real registry in the JVM suite.

Native factory bindings implement `Factories<S>` alongside timer `Context<S>`.
Their associated Argument type can retain domain-specific references and types;
the registry imposes no JSON argument format. Factories supply actual native
constructors, vararg interpretation and subtype predicates. All registry locks
are released before factory/log/content callbacks. Typed lookup stops at the
first match without copying the registry. This is an implementation detail,
not a measured throughput or concurrency-parity claim.

## Verification and remaining integration

```powershell
cargo test -p rs09-server --test timer_registry --test timers --test core_data
cargo xtask timer-registry-oracle --check --python <python> --java-home <jdk11>
```

226 scenarios run the complete original TimerRegistry, TimerManager, RSTimer,
PersistTimer and TimerFlag with verbatim ContentAPI wrappers. The oracle uses
actual Kotlin reflection, JSON-simple and HashMap, with scripted content
subclasses and explicit entity/clock/log bindings. Tests cover case differences,
Unicode, duplicate/null map entries, factory arguments/failed callbacks, return
type mismatches, automatic installation, persisted timers and collision/tree-bin
order. Observations include identities, every manager list, normalized map order,
logs, callbacks, constructor results and partial state after each action.

Oracle-only reflection resets an empty HashMap's table/threshold between cases
because `clear()` retains capacity. This provides independent fresh-world tests
without rewriting the original registry or changing final static references.
No reflection or JVM is used by native gameplay code.

Concrete timer classes, reflection-based plugin discovery, registry population at
world startup, Entity initialization, the running world scheduler and full player
construction remain pending. Rust subtype predicates are factory metadata checks,
not arbitrary callback predicates. Java object/argument APIs and concurrent raw
registry mutation are outside the claim. All source-file replacement statuses
remain `not_replaced` until complete runtime acceptance.

The registry uses [native JDK ROOT casing](string-case.md). Other locale-dependent
uses of String lowercasing require separate integration and evidence.

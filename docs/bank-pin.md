# Native bank PIN state and interface program

`server/player/bank_pin.rs` ports the full pinned BankPinManager and nested
PinStatus algorithms, plus BankPinInterface registration and button handling.
Canonical players own a real manager with its final player backreference. Close
events are distinct shared objects capturing the actual manager, so old events
continue to reset their original manager after a wire index or player-manager
field is reused. Stateless interface program instances also retain real object
identity. Services are required typed inputs; runtime code does not invoke Java.

The 855-case JVM corpus compiles the complete original manager with exactly seven
System.currentTimeMillis calls redirected to a controlled clock. The complete
BankPinInterface source is unchanged. It uses the real JDK11 Collections.shuffle
algorithm with its internal Random seeded through test-only reflection. Native
shuffle uses the shared JavaRandom implementation with one retained RNG. The
initial 836 observations are pinned independently and remain byte-identical.

Observations compare every action result, error category, full manager state,
UTF16 pin/message units, keypad permutation, manager/event alias, registry
program identity and callback prefix. Coverage includes choosing/confirming PINs,
rejected guesses, wrong attempts, try locks, PIN changes/deletion/cancellation,
recovery delays, status/settings/login messages, pending activation, parse
prefixes, old close callbacks, cross-player handlers, registration failures,
reentrant service callbacks, counter/clock overflow and failed operations.

Preserved quirks include the narrowing of pending milliseconds to int before
division, two separate clock reads in getPendingDays, the original `/100`
minutes conversion in the lock dialogue, strict activation/blocked comparisons,
sticky try counts, digit shuffles after entry, Java null string concatenation,
byte/char conversions in easy-guess checks, and Arrays.copyOf padding after a
reentrant temporary-PIN change. UTF16 storage preserves code-unit substring and
length behavior. Close callbacks reset changeState and deleting, while cancel
only resets the fields changed by the original doCancelPin.

The canonical ownership regression covers separate and aliased close events,
old-manager retention after disconnect/index reuse, changes applied to the old
player only, and final collection after the last event/view is released. Detached
primitive digit copies do not retain a player. Field guards never span external
services. Other existing ownership/player tests remain required.

Boundary scope remains explicit. Player/interface/component/packet/audio/varp/
GE/world services and RandomFunction samples are recording adapters in the JVM
corpus. Raw private initial states are fixture inputs. Their native production
counterparts are required by the Context traits, but full component close-event
and plugin dispatch, original RandomFunction distribution, real emitted frames,
PIN persistence in full profiles, default content registration and playable live
banking are not proven by these tests. Full reflection/subclass APIs and malformed
UTF16 through the shared bounded JSON parser remain separate. Do not treat the
manager kernel as finished live bank protection or a full Player replacement.

Audio constants resolve from the same separately pinned official ConstLib1.4.28
artifact as the bank core. The upstream dependency range and historical-version
limits remain unchanged. No speed, memory or capacity claim follows from this
checkpoint.

Component fields now use the actual typed PIN program and close-event headers.
Their canonical graph and guard dispatch have a separate351-case interface corpus
and native lifetime/runtime regressions; see docs/component-objects.md. Full live
PIN opening/entry and bank service composition are still pending.

```powershell
cargo test -p rs09-server --test bank_pin --test owned_player
cargo xtask bank-pin-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```

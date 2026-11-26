# Deferred input and bank transfer kernel

`world/input_prompt.rs` ports the modern RunScript.processInput algorithm, the
selected PacketProcessor.InputPromptResponse branch, ContentAPI input prompts
and BankUtils.transferX's captured callback. The original Java player/link script
class is a different API and is not used as the behavioral reference.

The1137-case JVM corpus compiles complete unchanged RunScript.kt, InputType.kt and
BankUtils.kt, and exact selected prompt/packet bodies. A separate initial lock
pins the first790 observations byte-for-byte; a separate wire lock pins first1131.
The extension uses unchanged original244/65/23 decoder and primitive/string/name
bodies, including truncation cursor prefixes, offsets, tails and base37 edges.
Pinned declared kotlin-reflect1.5.20 makes the actual callback text
`(kotlin.Any) -> kotlin.Unit`; stdlib-only reflection produces a different string.
Player/attribute/lock/interpreter/bank
and named callback implementations are explicit mutable recording services.
Native replay uses actual retained NodeHandles in captured Transfer objects;
recording bank calls are not a replacement for actual item mutation or a combined
whole-server JVM comparison.

Preserved behavior includes:

- Raw Int<=0 returns before attribute reads. Nonpositive text or Long values
  follow their own parsing path. Input type and parseamount have separate typed
  fallback reads and failures.
- Amount text lowercases with the pinned ROOT rules, matches ASCII Java digits
  and repeated k or m suffixes, then expands every suffix character. Thus1kk can
  become1000000; mixed1km is invalid. The Kelvin sign can lowercase to k. Numeric
  conversion preserves original values when Kotlin Int parsing fails, including
  overflow and unpaired UTF16 input. Non-amount numeric text retains JDK digit
  parsing, without trimming.
- Callback false, true and Kotlin Unit results are discarded. NumberFormat and
  ClassCast callback failures send different required dialogue messages. Earlier
  attribute/toString/parse failures do not enter that catch/finally.
- Inner cleanup removes runscript then input-type. The packet outer finally
  removes parseamount then runscript, including early parser returns. Missing or
  wrong script and an interaction lock can exit before outer cleanup. A cleanup
  failure replaces an earlier outcome and skips subsequent cleanup calls.
- Amount prompts set parseamount before the actual interpreter lookup/send;
  other prompt types select their corresponding108/109/110/107 service. Script
  and input-type attributes are stored after the send. Mutation-before-event
  failures preserve the completed attribute prefix.
- Transfer captures its actual player, slot, withdrawal flag and optional after
  function. It parses Integer, gets the current bank for transfer, gets it again
  for last-X, and invokes after last. Retained aliases continue to use that
  captured player when invoked from another player's prompt.

Canonical attributes can now store actual object aliases with `set_value`.
AttributeObject.captures and NativeObject.captures report physical strong fields.
The existing component graph also traces attribute objects and nested native
headers. Each actual header is inspected once; separate map/child fields count
separately. External aliases root only the players captured by their contents.
Returned stateless attribute objects do not anchor the player that stored them.
Metadata inspection runs after player field guards are released.

Eight new ownership regressions cover self and cross-player attribute cycles,
duplicate aliases/fields, nested shared native headers, replacement with exposed
old values, stateless values, metadata reentry for reads and the actual Transfer
player/after fields and native/attribute trait views of the same Arc allocation.
The latter use one allocation identity and inspect its physical captures once;
both views must report those same actual fields. These collect cycles through
canonical players; they do not
establish reclamation of arbitrary opaque-only Arc cycles or concurrent GC.
Concrete callback execution must reject foreign worlds before gameplay mutation.
Scalar Display attribute objects retain the existing empty-capture default;
new native payloads with strong fields must supply their metadata explicitly.

The recording harness's first snapshot retained a mutable transfer list. Native
comparison exposed this as an earlier observation changing after a later call.
The recorder now copies that list; original source is unchanged. The failed
authoring attempt is preserved under runs/bank-input-investigation/authoring-1.

The controlled runtime now stores actual typed Function1 attribute objects and
executes BankUtils transfers against the captured canonical player.244 base37,
65 signed-byte terminated text and23 numeric responses use the shared decoder.
The plain error sender uses actual171 text, manager chatbox open and hidden
packets;132 Continue closes overlay752:12 and restores137 while bank762 stays
open. Five regressions in each live example cover transfer/cleanup, actual error
and Continue, pending callback collection, foreign capture rejection and retained
callbacks after Close. A late packet after Close has no current script and leaves
parseamount; an externally retained function still transfers into the closed bank,
matching the original BankContainer methods without an open-state guard.

`pickup_agent --banking-x` independently passes finite real TCP and WebSocket
runs: real ground1351 pickup, Deposit-X1k, Withdraw-X1, invalid oops, overflow
2147483648, each Continue, Close184, ignored late155 and at least two later225
updates. It requires four actual109 prompts, twelve requests, last-X1249 values
50→1000→1 and unchanged items after both errors. A continuously-reading spectator
receives no private bank, input or pickup sound. Evidence is in
`runs/live-bank-input-investigation/runtime-x`; only its owned server was retired.
The separate frozen visual candidate has actual browser pointer/keyboard and
protocol-assisted Windows prompt receipts in
`runs/client-live-bank-input-validation`. Native OS input is not established.

Fresh admission explicitly has empty interaction locks, action queue and no active
content dialogue. Arbitrary Function1 bodies/after callbacks, full Entity events,
natural banker/booth/PIN admission, profiles and complete content remain required.
See [plain dialogue scope](plain-dialogue.md). No full replacement, measured speedup
or capacity claim is made; all4042 whole-source rows remain not_replaced.

`rs09-server --world` installs this kernel (`world/live/talk.rs`, [the live world](live-world.md)): the InputPromptResponse branch through `receive`, `sendInputDialogue` through `send` with the live dialogue interpreter, and `BankUtils.transferX` from the bank's buttons; the callbacks are the bank's transfer object, the shops' and the trade's amount scripts and the dialogue DSLs' closures. A transfer's after callback is refused there.

```powershell
cargo test -p rs09-server --test input_prompt --test plain_dialogue --test owned_player
cargo xtask input-prompt-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```

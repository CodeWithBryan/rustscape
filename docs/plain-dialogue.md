# Plain dialogue and Continue

`world/plain_dialogue.rs` ports the complete finite positive-limit DialUtils
splitting algorithm and selected plain DialogueInterpreter sender. The430-case
JVM corpus compiles unchanged DialUtils.kt and exact selected Java/Kotlin sender
and PacketProcessor.ContinueOption bodies. Player/packet/interpreter/manager,
queue, listener and plugin services are explicit mutable recording adapters.
An independent lock preserves the first424 observations.

Splitting retains UTF16 lengths, ASCII tag recognition, float-ceil initial size,
spaces/empty lines, the original closing-tag substring quirk, first-equal tag
removal during reverse traversal, and merging only lines four/five before
dropping subsequent lines. For an overlong bare token, the original repeatedly
retries until its signed accumulator wraps. Rust skips those identical pure
retries and preserves the resulting output, independently compared with the JVM.
No measured performance claim follows from that algorithm change.

Nonpositive limits and nonprogressing tagged text such as `<b> abcdef` at limit4
return explicit binding errors. The reference can exhaust resources in those
cases; that failure behavior is not reproduced or claimed as complete parity.

The sender retains the actual mutable message array, captures its initial count
and reads each current element after obtaining the dispatch receiver. Counts
outside1..4 send nothing. It preserves captured interpreter versus late player,
manager/chatbox getters, text-before-open-before-hidden order and completed
failure prefixes. ContentAPI captures its interpreter before splitting. No guard
spans a callback. Java oracle fields are private where Kotlin must call getters;
a failed adapter draft that bypassed getters is preserved under the investigation.

Continue checks the first interpreter, closes the chatbox if no active dialogue,
then re-reads the interpreter before popping an action. It resolves the current
manager/component and listener; a false listener result triggers a late plugin
lookup. Active dialogue also re-reads its interpreter. Queue aliases, null entries,
listener/plugin replacements, reentry and exceptions are compared with the JVM.

The controlled runtime uses actual text171, chatbox open/hidden and132 packets.
Continue closes overlay752:12, retains base137 and leaves bank762 open. Fresh
players explicitly have no queued content actions or active dialogue. Their
implementations and arbitrary dialogue/plugin content remain required; the oracle
is not a combined full-server or full-client comparison. See
[live input acceptance](input-prompt.md).

```powershell
cargo test -p rs09-server --test plain_dialogue --test input_prompt --test interfaces --test bank_interface
cargo xtask plain-dialogue-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```

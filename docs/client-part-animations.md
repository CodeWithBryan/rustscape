# Player and NPC part animations

The shared Windows/WASM body renderer now consumes the twelve slot tracks from
the existing player and NPC animation state. Slot calls run in ascending order,
before the action/movement blend, and each complete GL call retains the original
four fractional bits and final rounding. Slot delay does not suppress drawing
frame zero. Resize and terrain alignment remain after animation.

Body assembly retains the original sparse slot indices. Equal-coordinate welded
vertices OR their source bits; triangle sources retain their own slot, including
texture-only vertex references. A missing source array means unmasked geometry,
whereas a present zero source is excluded by masked transforms. NPC single-model
assembly remains distinct from combining one present model in a sparse array.

`animation_parts.rs` ports the complete GL masked bone arithmetic, including the
optional fifteen-integer BAS basis, wrapped integer intermediates, alpha/HSL
changes, and per-axis signed-short normal truncation. An explicit renderer-owned
boolean preserves the original GL origin-valid state across body calls. Matrix
rotation/scaling does not update normals even when requested: that is original
behavior. Full software masked rendering, static model-cache sharing, malformed
topology/failure prefixes and the original GPU dirty-buffer scheduler are outside
this change.

The separate `animations.pack` contains all 11,155 sequences, 219,678 frames and
2,435 bases from the pinned archives. Metadata is available before request
priority/replay decoding; active frame/base dependencies are decoded on demand
before clocks and drawing. Existing prepared UI/world overrides keep precedence.
An unavailable full pack reports the bounded subset fallback. Loading failures
remain visible diagnostics. `tools/prepare_client_animations.py` verifies cache
hashes before creating a fresh pack and never replaces closed UI/world inputs.
Read-only `slot_poses` snapshots record actor identity, selected tracks, source
coverage and the body vertex checksum before resize/terrain; these observations
do not choose gameplay or dispatch requests.

The unchanged original Java is used only to generate comparison fixtures:

- `oracle/players/masked.py`: 2,048 sequences, 16,384 individual transforms, and
  512 basis inputs independently evaluated through both PlayerAppearance and
  NpcType matrix construction branches.
- `oracle/players/parts_corpus.py`: 512 real-cache sparse model assemblies and
  1,792 complete sequential masked/full/dual GL calls. It covers sparse and
  repeated parts, welded source bits, shift/short truncation beyond sixteen and
  thirty-two slots, authored base masks and BAS matrices, tweening, normals,
  alpha/color and retained origin-valid state.
- `part_pose_tests.rs`: 256 of those cases pass through the actual main renderer
  slot resolver and body animation binding, including positive slot delays.
- `oracle/players/animation_assets.py`: every packaged sequence/frame/base is
  compared with the complete unchanged original decoders and SeqType.postDecode,
  including part masks, frame flags, secondary frames and sound arrays.

Reference source and fixture asset hashes are checked. Triangle identity tags in
the oracle adapter are observation-only: the adapter restores original reordered
source masks before invoking unchanged animation methods. This corpus does not
establish full PlayerAppearance/NpcType cache ownership or full client parity.

`tools/prepare_slot_replay.py` supplies a bounded rendering preview. NPC input
uses actual NPC_INFO32 slot-mask bytes; player input is explicitly authored offline
self requests, rather than a PLAYER_INFO wire test. Head gestures exercise delay,
part selection, replacement and clear. Arbitrary whole-body emotes applied as
individual-part overlays can produce unnatural poses in the original algorithm;
those combinations remain compatibility stress cases rather than a gameplay demo.

Natural banker/booth/PIN admission, live shops, remaining interface behavior,
dialogue lifecycle, modern interaction arrival, projectiles/particles, higher
floors and native pointer acceptance remain separate work. A completed part
rendering slice does not establish those behaviors or a percentage of parity.

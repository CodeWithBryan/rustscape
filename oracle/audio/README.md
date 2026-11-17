# Original client audio observations

Run `python oracle/audio/check.py` with Python3.10+ and JDK25. Java compiles only
under `runs/`; the native client and packaged builds contain no JVM.

The PCM, location ambient, cached metadata/synthesis and active-varp probes retain
their earlier observations. `npc.py` compiles unchanged `AreaSound` methods and
the NPC branches of `AreaSoundManager.add/remove/redraw/updateMulti/clear`, plus
`Npc.getSound`, original linked lists and primitive BAS defaults. Explicit
resolved NPC definitions, movement sequences, cache readiness and a recording
mixer are external services. Location/player cohorts are empty in this probe.

The66 scenarios compare4,389 ordered command results: reference identity and
duplicate collection entries, index reuse, retained old actors, first-reference
removal, morph refreshes, null-morph defaults, movement-ID aliases, category-only
sound updates, stale radius behavior, attenuation, mute/plane/linkage and clears.
The cached437 BAS alias case retains idle/walk2150 and missing run IDs.

The separate metadata probe runs the complete original NpcType decoder against
all8,590 unchanged streams in the prior NPC fixture; its whole input-file hash is
retained. All19 distinct effects referenced by the71 audible cached NPC types
are synthesized by unchanged original instrument/filter/envelope methods at
22050Hz, before and after queue trimming; no groups are missing. The optional
NPC asset generator adds40 original GL geometry/pose/shadow comparisons for the
two cached mine-cart dwarf types without changing existing player/NPC fixtures.

`replay.py` compares a real Windows command trace/WAV with unchanged original
SoundPcmStream and JavaAudioChannel.write. Captured native samples are the input
to this mixer replay; raw cache synthesis is checked independently. The current
five-voice NPC run matches678,912 PCM bytes exactly and stops every started NPC
voice after type replacement/removal. GPU frames and the real-time PCM run are
separate recordings. Hardware loopback, music, global scheduling, player ambient,
browser continuous playback and full audio/gameplay parity are outside this claim.

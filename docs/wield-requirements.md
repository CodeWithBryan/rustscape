# The wield and use skill requirements (`ItemDefinition.hasRequirement`)

`world/packet_handlers/requirement.rs` ports
`ItemDefinition.hasRequirement(player, wield, message)`: the item
configuration's `requirements` map (skill to level) walked in JDK `HashMap`
key order (`Requirements::order`, the configuration's insertion order), keys
outside `Skills.SKILL_NAME` (24 names) skipped, the first skill whose static
level is below the requirement failing the check with
`"You need a[n] <Skill> level of <level> to wear|use this."` when asked. A
configuration of another class fails as the original's cast. EquipHandler's
`EquipmentContainer.add` in the live world (`world/live/items.rs`) calls it
over the canonical skills and sends the message through the player's packet
dispatch.

## Reference tests

```powershell
cargo test -p rs09-server --test wield_requirements
cargo xtask wield-requirements-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/wield-requirements/oracle.jsonl`: 1,146 cases. The oracle
(`oracle/wield-requirements`) compiles `hasRequirement` verbatim from the
pinned `ItemDefinition.java` (extracted, not rewritten), with
`Skills.SKILL_NAME` and `StringUtils.isPlusN` verbatim, on JDK 11; the
configuration (absent, a `String`, or a `java.util.HashMap` filled in the
case's order), `Skills.getStaticLevel`, `Player` and
`PacketDispatch.sendMessage` are adapters. The cases cover the absent,
empty and wrongly typed configuration, every skill below, at and above its
requirement with both verbs and both message flags (the "an" names), keys
outside the name array (negative, 24, 25, the integer limits) before and
after valid keys, level boundaries, bucket collisions, keys past the table,
resizes past twelve entries with forward and reversed insertion, and 300
seeded random maps. The native test replays every case and compares the
result, the exception class and the messages.

The first replay found a defect in the earlier native port: it accepted
skill 24 (`0..25`), past `SKILL_NAME` and the 24 static levels, where the
original skips it.

## Not covered

Null map values and non-`Integer` keys (the native configuration holds int
pairs), and the item configuration parser itself (covered by item-config).

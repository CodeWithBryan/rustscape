package core.cache.def.impl;

import oracle.Support;

/** forId and getName record; the name is a case input ("NPC id" when absent). */
public class NPCDefinition {
	private final int id;

	private NPCDefinition(int id) {
		this.id = id;
	}

	public static NPCDefinition forId(int id) {
		Support.emit("npcDefinition", id);
		return new NPCDefinition(id);
	}

	public String getName() {
		Support.emit("npcName", id);
		return Support.npcName(id);
	}
}

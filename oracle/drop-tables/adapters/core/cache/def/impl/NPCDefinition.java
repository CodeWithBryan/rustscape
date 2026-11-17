package core.cache.def.impl;

import core.game.node.entity.npc.drop.NPCDropTables;

/** Lookup service for the drop-table oracle. The dropTables field declaration is the original's. */
public class NPCDefinition {
	public final int id;
	public String name;
	public int combatLevel;
	private NPCDropTables dropTables = new NPCDropTables(this);

	public NPCDefinition(int id) {
		this.id = id;
	}

	public static NPCDefinition forId(int id) {
		return oracle.Support.forId(id);
	}

	public NPCDropTables getDropTables() {
		return dropTables;
	}

	public void setDropTables(NPCDropTables dropTables) {
		this.dropTables = dropTables;
	}

	public String getName() {
		return name;
	}

	public int getCombatLevel() {
		return combatLevel;
	}
}

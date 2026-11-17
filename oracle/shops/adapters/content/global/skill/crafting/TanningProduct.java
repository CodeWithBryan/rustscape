package content.global.skill.crafting;

/** TanningProduct.open as a recorded service (content). */
public enum TanningProduct {
	;

	public static void open(final core.game.node.entity.player.Player player, final int npc) {
		oracle.Support.emit("tanning", oracle.Support.index(player), npc);
	}
}

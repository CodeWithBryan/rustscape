package core.game.node.entity.skill;

import core.game.node.entity.player.Player;
import java.util.Map;
import oracle.Support;

/** The skill ids (original text); refresh, getLevel and addExperience record. */
public class Skills {
	/* ORIGINAL_SKILL_IDS */

	private final Player owner;
	private final Map levels;

	public Skills(Player owner, Map levels) {
		this.owner = owner;
		this.levels = levels;
	}

	public void refresh() {
		Support.emit("skillsRefresh", owner.slot);
	}

	public int getLevel(int slot) {
		Support.emit("level", owner.slot, slot);
		return Support.num(levels, Integer.toString(slot), 1);
	}

	public void addExperience(int slot, double experience) {
		Support.emit("addExperience", owner.slot, slot, Support.dbl(experience));
	}
}

package content.global.activity.ttrail;

import core.game.node.item.Item;
import core.tools.Log;
import core.tools.RandomFunction;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static core.api.ContentAPIKt.log;

/**
 * The registered clue scrolls of a level are an input of each case. getClue is
 * the original text, inserted at generation time.
 */
public class ClueScrollPlugin {
	public static final Map<ClueLevel, List<ClueScrollPlugin>> ORGANIZED = new HashMap<>();
	private final int clueId;

	public ClueScrollPlugin(int clueId) {
		this.clueId = clueId;
	}

	public int getClueId() {
		return clueId;
	}

	/* ORIGINAL_GET_CLUE */
}

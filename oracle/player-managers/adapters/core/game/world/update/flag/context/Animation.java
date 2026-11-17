package core.game.world.update.flag.context;

/** An animation id. */
public class Animation {
	private final int id;

	private Animation(int id) {
		this.id = id;
	}

	public static Animation create(int id) {
		return new Animation(id);
	}

	public int getId() {
		return id;
	}
}

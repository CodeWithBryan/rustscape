package core.game.world.update.flag.context;

/** A graphics id. */
public class Graphics {
	private final int id;

	private Graphics(int id) {
		this.id = id;
	}

	public static Graphics create(int id) {
		return new Graphics(id);
	}

	public int getId() {
		return id;
	}
}

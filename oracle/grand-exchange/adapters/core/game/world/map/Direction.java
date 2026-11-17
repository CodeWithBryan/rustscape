package core.game.world.map;

/** Direction's step fields, values as in the original enum (traversal masks not used here). */
public enum Direction {
	NORTH_WEST(-1, 1), NORTH(0, 1), NORTH_EAST(1, 1), WEST(-1, 0), EAST(1, 0), SOUTH_WEST(-1, -1), SOUTH(0, -1), SOUTH_EAST(1, -1);

	private final int stepX;
	private final int stepY;

	Direction(int stepX, int stepY) {
		this.stepX = stepX;
		this.stepY = stepY;
	}

	public int getStepX() {
		return stepX;
	}

	public int getStepY() {
		return stepY;
	}
}

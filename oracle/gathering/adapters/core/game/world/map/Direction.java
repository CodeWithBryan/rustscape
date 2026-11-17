package core.game.world.map;
/** Carrier for the declared step vector of player.direction; not the original enum. */
public class Direction {
    private final int stepX, stepY;
    public Direction(int stepX, int stepY) { this.stepX = stepX; this.stepY = stepY; }
    public int getStepX() { return stepX; } public int getStepY() { return stepY; }
}
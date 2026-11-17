package core.game.world;
/** Controlled tick/identity inputs, with no simulated gameplay. */
public final class GameWorld {
    public static int ticks;
    public static int getTicks() {return ticks;}
    public static Settings getSettings() {return new Settings();}
    public static final class Settings {public int getWorldId() {return 1;}}
}

package core.game.system.task;

/** Construction only: scheduling is the pulse port's; the case calls pulse(). */
public abstract class Pulse {
	public Pulse() {
	}

	public abstract boolean pulse();
}

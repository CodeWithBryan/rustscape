package core.game.system.task;

import core.game.node.Node;

/** Pulse construction only: scheduling is the pulse port's, not compared here. */
public abstract class Pulse {
	public final int delay;
	public final Node[] checks;

	public Pulse(int delay, Node... checks) {
		this.delay = delay;
		this.checks = checks;
	}

	public abstract boolean pulse();
}
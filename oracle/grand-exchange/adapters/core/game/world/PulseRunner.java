package core.game.world;

/** GameWorld.Pulser: submissions are recorded and run by the case's "pulse" action. */
public class PulseRunner {
	public void submit(core.game.system.task.Pulse pulse) {
		oracle.Support.submit(pulse);
	}
}
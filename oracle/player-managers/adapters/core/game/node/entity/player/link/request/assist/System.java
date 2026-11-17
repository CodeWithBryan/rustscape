package core.game.node.entity.player.link.request.assist;

/**
 * Same-package classes shadow java.lang: AssistSession's System is this
 * clock, the case's current time, recorded on each read. The source text of
 * AssistSession is unchanged.
 */
public class System {
	public static long currentTimeMillis() {
		oracle.Support.emit("clock");
		return oracle.Support.clock;
	}
}

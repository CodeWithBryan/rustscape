package rt4;

/** Oracle adapter: a scripted clock, so that click times are reproducible. */
public final class MonotonicClock {
	public static long now;

	public static long currentTimeMillis() {
		return now;
	}
}

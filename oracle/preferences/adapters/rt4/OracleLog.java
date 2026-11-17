package rt4;

import java.util.ArrayList;
import java.util.List;

/** Oracle adapter: the ordered record of what the unchanged classes asked the rest of the client to do. */
public final class OracleLog {
	public static final List<String> events = new ArrayList<>();

	public static void add(String event) {
		events.add(event);
	}
}
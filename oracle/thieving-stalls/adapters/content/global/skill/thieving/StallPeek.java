package content.global.skill.thieving;
/** Adapter: reads the package-private field the driver lists. */
public final class StallPeek {
    public static String msgItem(Stall stall) { return stall.msgItem; }
    public static java.util.List<Integer> emptyIds(Stall stall) { return stall.empty_ids; }
}

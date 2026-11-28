package content.data.consumables;
import oracle.W;
/** Adapter: getConsumableById answers from the case's declared consumables (id to its consumable's ids). */
public final class Consumables {
    private final int[] ids;
    private Consumables(int[] ids) { this.ids = ids; }
    public Consumable getConsumable() { return new Consumable(ids); }
    public static Consumables getConsumableById(final int itemId) {
        int[] ids = W.consumable(itemId);
        return ids == null ? null : new Consumables(ids);
    }
    public static final class Consumable {
        private final int[] ids;
        Consumable(int[] ids) { this.ids = ids; }
        public int[] getIds() { return ids; }
    }
}

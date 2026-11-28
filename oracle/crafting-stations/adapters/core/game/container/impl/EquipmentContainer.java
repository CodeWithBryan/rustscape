package core.game.container.impl;
import core.game.node.item.Item;
import oracle.W;
/** Adapter: the pinned slot constant and the declared hat. */
public final class EquipmentContainer {
    /* SLOT_HAT */
    public Item get(int slot) {
        W.emit("equipment", slot);
        Object hat = W.state.get("hat");
        return hat == null ? null : new Item(((Number) hat).intValue());
    }
}

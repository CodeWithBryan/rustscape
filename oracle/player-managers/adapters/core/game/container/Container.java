package core.game.container;

import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import java.util.ArrayList;
import java.util.List;
import oracle.Support;

/**
 * A player's inventory, equipment or bank as the case holds it: slots of
 * (id, amount). Every member records its call. The host model behind the
 * answers: containsItem sums the id's amounts, remove takes the amount from
 * the first matching slots when contained, add fills the first empty slot.
 */
public class Container {
	private final Player owner;
	private final String kind;
	private final Item[] items;
	private final Listeners listeners = new Listeners();

	public class Listeners extends ArrayList<ContainerListener> {
		@Override
		public boolean add(ContainerListener listener) {
			Support.emit("listenerAdd", owner.slot, kind, Support.value(listener));
			return super.add(listener);
		}

		@Override
		public boolean remove(Object listener) {
			Support.emit("listenerRemove", owner.slot, kind, Support.value(listener));
			return super.remove(listener);
		}
	}

	public Container(Player owner, String kind, int capacity, Object spec) {
		this.owner = owner;
		this.kind = kind;
		this.items = new Item[capacity];
		if (spec != null) {
			for (Object row : (List) spec) {
				List r = (List) row;
				items[Support.num(r.get(0))] = new Item(Support.num(r.get(1)), Support.num(r.get(2)));
			}
		}
	}

	public String kind() {
		return kind;
	}

	public Item[] raw() {
		return items;
	}

	public List<ContainerListener> getListeners() {
		Support.emit("listeners", owner.slot, kind);
		return listeners;
	}

	public List<ContainerListener> rawListeners() {
		return listeners;
	}

	public Item[] toArray() {
		Support.emit("toArray", owner.slot, kind);
		return items.clone();
	}

	public Item get(int slot) {
		Support.emit("get", owner.slot, kind, slot);
		return slot < 0 || slot >= items.length ? null : items[slot];
	}

	private int amount(int id) {
		int total = 0;
		for (Item item : items) {
			if (item != null && item.getId() == id) {
				total += item.getAmount();
			}
		}
		return total;
	}

	public boolean containsItem(Item item) {
		Support.emit("contains", owner.slot, kind, item.getId(), item.getAmount());
		return amount(item.getId()) >= item.getAmount();
	}

	public boolean remove(Item item) {
		Support.emit("remove", owner.slot, kind, item.getId(), item.getAmount());
		if (amount(item.getId()) < item.getAmount()) {
			return false;
		}
		int left = item.getAmount();
		for (int i = 0; i < items.length && left > 0; i++) {
			if (items[i] != null && items[i].getId() == item.getId()) {
				int take = Math.min(left, items[i].getAmount());
				left -= take;
				items[i] = items[i].getAmount() == take ? null : new Item(item.getId(), items[i].getAmount() - take);
			}
		}
		return true;
	}

	public boolean add(Item item) {
		Support.emit("add", owner.slot, kind, item.getId(), item.getAmount());
		for (int i = 0; i < items.length; i++) {
			if (items[i] == null) {
				items[i] = new Item(item.getId(), item.getAmount());
				return true;
			}
		}
		return false;
	}

	public int freeSlots() {
		Support.emit("freeSlots", owner.slot, kind);
		int free = 0;
		for (Item item : items) {
			if (item == null) {
				free++;
			}
		}
		return free;
	}
}

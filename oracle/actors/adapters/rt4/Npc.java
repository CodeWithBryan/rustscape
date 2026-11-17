package rt4;

/** Oracle adapter: an NPC as a PathingEntity with a type; no model or rendering. */
public class Npc extends PathingEntity {
	public NpcType type;

	public int getBasId() {
		return this.basTypeId;
	}

	public boolean isVisible() {
		return this.type != null;
	}

	public void lean(Model model) {
		this.applyBodyLean(model);
	}

	public void updateModel(int a, int b, int c, int d, int e) {
	}

	public void render(int a, int b, int c, int d, int e, int f, int g, int h, long key, int i, ParticleSystem particles) {
	}

	public int getMinY() {
		return this.minY;
	}
}
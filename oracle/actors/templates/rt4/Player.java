package rt4;

import org.openrs2.deob.annotation.OriginalArg;
import org.openrs2.deob.annotation.OriginalMember;
import org.openrs2.deob.annotation.Pc;

/**
 * Oracle adapter: a player as a PathingEntity. `animate` below is copied
 * verbatim from the reference Player.java when the oracle is generated; the
 * rest of that class (appearance, rendering, sounds) is not compiled.
 */
public class Player extends PathingEntity {
	public static int plane;
	public boolean lowDetail = false;
	public boolean visible = true;

	public int getBasId() {
		return this.basTypeId;
	}

	public boolean isVisible() {
		return this.visible;
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

	//@EXTRACT Player.java public static void animate(
}

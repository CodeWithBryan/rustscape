package rt4;

import java.util.ArrayList;
import java.util.List;

/**
 * Oracle adapter: records the transforms actor code applies to a body model.
 * Operation codes: 0 translate-y, 1 rotateX, 2 rotateZ, 3 rotateY.
 */
public class Model extends Entity {
	public static final List<int[]> operations = new ArrayList<int[]>();
	public static int minimum;
	public boolean pickable;

	public int getMinY() {
		return minimum;
	}

	public void translate(int x, int y, int z) {
		if (x != 0 || z != 0) {
			throw new IllegalStateException("unexpected horizontal translation");
		}
		operations.add(new int[] {0, y});
	}

	public void rotateX(int angle) {
		operations.add(new int[] {1, angle});
	}

	public void rotateZ(int angle) {
		operations.add(new int[] {2, angle});
	}

	public void rotateY(int angle) {
		operations.add(new int[] {3, angle});
	}

	public void rotate180() {
	}

	public void rotateClockwise() {
	}

	public void rotateCounterClockwise() {
	}

	public Model copyForAnimation(boolean a, boolean b, boolean c) {
		return this;
	}

	public Model copyForEntity(boolean a, boolean b, boolean c) {
		return this;
	}

	public Model copyForLoc(boolean a, boolean b, boolean c) {
		return this;
	}

	public void applyAnimation(AnimFrameset a, int b, AnimFrameset c, int d, int e, int f, boolean g) {
	}

	public void updateModel(int a, int b, int c, int d, int e) {
	}

	public void render(int a, int b, int c, int d, int e, int f, int g, int h, long key, int i, ParticleSystem particles) {
	}
}
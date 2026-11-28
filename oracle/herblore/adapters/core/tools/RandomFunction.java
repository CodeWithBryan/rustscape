package core.tools;
import oracle.W;
/** Adapter with the original's bodies; RANDOM.nextInt is the case's next declared draw. */
public final class RandomFunction {
    public static final int random(int a, int b) {
        final int n = Math.abs(b - a);
        return Math.min(a, b) + (n == 0 ? 0 : random(n));
    }
    public static final int random(int maxValue) {
        if (maxValue <= 0) {
            return 0;
        }
        return W.draw(maxValue);
    }
}

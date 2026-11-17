import rt4.*;
import java.util.*;

/** Isolated primitive measurements; excludes process startup, includes object initialization. */
public final class OracleBenchmark {
    private static volatile long sink;
    private static long workload(String name, int count) {
        long checksum = 0;
        if (name.equals("isaac_keys")) {
            IsaacRandom random = new IsaacRandom(new int[] {1, 2, 3, 4});
            for (int i = 0; i < count; i++)
                checksum = checksum * 31 + Integer.toUnsignedLong(random.getNextKey());
        } else {
            byte[] input = new byte[1024];
            for (int i = 0; i < input.length; i++) input[i] = (byte) (i * 17 + i / 7);
            Buffer buffer = new Buffer(input);
            for (int i = 0; i < count; i++) {
                if (buffer.offset == input.length) buffer.offset = 0;
                checksum = checksum * 31 + Integer.toUnsignedLong(buffer.g4());
            }
        }
        return checksum;
    }
    public static void main(String[] args) {
        GlobalConfig.USE_ISAAC = true;
        int count = Integer.parseInt(args[0]);
        for (String name : new String[] {"isaac_keys", "buffer_g4"}) {
            for (int i = 0; i < 20; i++) sink = workload(name, count);
            long[] samples = new long[9];
            long checksum = 0;
            for (int i = 0; i < samples.length; i++) {
                long start = System.nanoTime();
                checksum = workload(name, count);
                sink = checksum;
                samples[i] = System.nanoTime() - start;
            }
            System.out.println("{\"name\":\"" + name + "\",\"checksum\":\""
                + Long.toUnsignedString(checksum) + "\",\"samplesNs\":"
                + Arrays.toString(samples) + "}");
        }
    }
}

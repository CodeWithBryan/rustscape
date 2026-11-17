package oracle.worldtrace;

import java.lang.reflect.Field;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.ZoneOffset;
import java.util.Calendar;
import java.util.Random;
import java.util.concurrent.ThreadLocalRandom;
import java.util.concurrent.atomic.AtomicLong;
import sun.misc.Unsafe;

/**
 * The recorded replacements for the inputs the original reads from its host:
 * the wall clock, unseeded generators and the three shared random sources.
 * Rewrite.java redirects the original's call sites here; nothing in this
 * class is game logic.
 */
public final class Pin {
    /** java.util.Random with a fixed seed whose draws are counted. */
    public static final class Counting extends Random {
        private final AtomicLong draws = new AtomicLong();
        private final AtomicLong state;
        public Counting(long seed) {
            super(seed);
            try {
                Field f = Random.class.getDeclaredField("seed");
                state = (AtomicLong) U.getObject(this, U.objectFieldOffset(f));
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException(e);
            }
        }
        // Counted only once construction has finished (Random's constructor does not draw).
        @Override protected int next(int bits) {
            if (draws != null) draws.incrementAndGet();
            return super.next(bits);
        }
        public long draws() { return draws.get(); }
        public String state() { return String.format("%012x", state.get()); }
    }

    public static final Unsafe U;
    static {
        try {
            Field f = Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            U = (Unsafe) f.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static final long TICK_MILLIS = 600L;
    public static volatile long now;
    public static final AtomicLong clockReads = new AtomicLong();
    public static final AtomicLong seeds = new AtomicLong();
    public static final AtomicLong unpinned = new AtomicLong();
    public static Counting shared, math, shuffle;
    private static long tlrOffset;
    private static Thread main;

    public static void install(Field field, Object value) {
        U.putObject(U.staticFieldBase(field), U.staticFieldOffset(field), value);
    }

    /** Installs the generators. Each owning class is initialised first so its initialiser cannot overwrite them. */
    public static void installGenerators(long clockStart, long tlrSeed) throws ReflectiveOperationException {
        now = clockStart;
        main = Thread.currentThread();
        shared = new Counting(0L);
        math = new Counting(0L);
        shuffle = new Counting(0L);
        install(Class.forName("core.tools.RandomFunction").getDeclaredField("RANDOM"), shared);
        install(Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator"), math);
        java.util.Collections.shuffle(new java.util.ArrayList<Object>());
        install(java.util.Collections.class.getDeclaredField("r"), shuffle);
        ThreadLocalRandom.current();
        tlrOffset = U.objectFieldOffset(Thread.class.getDeclaredField("threadLocalRandomSeed"));
        U.putLong(main, tlrOffset, tlrSeed);
    }

    /** The calling (world) thread's ThreadLocalRandom seed: its position. */
    public static String threadLocalState() { return String.format("%016x", U.getLong(main, tlrOffset)); }

    // ---- call-site targets written by Rewrite.java -------------------------
    public static long currentTimeMillis() { clockReads.incrementAndGet(); return now; }
    public static long nanoTime() { clockReads.incrementAndGet(); return now * 1_000_000L; }
    public static Calendar pin(Calendar calendar) { clockReads.incrementAndGet(); calendar.setTimeInMillis(now); return calendar; }
    public static Instant instantNow() { clockReads.incrementAndGet(); return Instant.ofEpochMilli(now); }
    public static LocalDateTime localDateTimeNow() { return LocalDateTime.ofInstant(instantNow(), ZoneOffset.UTC); }
    public static LocalDate localDateNow() { return localDateTimeNow().toLocalDate(); }
    public static LocalTime localTimeNow() { return localDateTimeNow().toLocalTime(); }
    /** Seed for a generator the original constructs without one: the n-th such construction gets n. */
    public static long nextSeed() { return seeds.incrementAndGet(); }
    /** A source this harness cannot pin was constructed (SecureRandom). The run is rejected if this is ever non-zero. */
    public static void unpinned() { unpinned.incrementAndGet(); }
}

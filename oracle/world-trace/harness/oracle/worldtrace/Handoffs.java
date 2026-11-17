package oracle.worldtrace;

import com.google.protobuf.Message;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.LinkedBlockingDeque;
import kotlin.coroutines.CoroutineContext;
import kotlinx.coroutines.CoroutineDispatcher;

/**
 * The original hands work to other threads in two ways that matter here:
 * GlobalScope.launch (TaskExecutor, the management-event loop) and the
 * management-event queue. Both are pinned so that the work runs on the world
 * thread, in arrival order, at the points where the harness pumps.
 */
public final class Handoffs {
    /** Stands in for Dispatchers.Default: blocks wait here until the world thread pumps them. */
    public static final class Queue extends CoroutineDispatcher {
        private final ArrayDeque<Runnable> blocks = new ArrayDeque<>();
        @Override public void dispatch(CoroutineContext context, Runnable block) {
            synchronized (LOCK) {
                blocks.add(block);
                dispatched++;
                if (Boolean.TRUE.equals(TAKEN.get())) {
                    TAKEN.set(Boolean.FALSE);
                    arrived++;
                } else if (Thread.currentThread() != world) {
                    foreign.add(Thread.currentThread().getName());
                }
                LOCK.notifyAll();
            }
        }
    }

    /** Stands in for ManagementEvents.eventQueue: take() returns only when the world thread grants it. */
    public static final class Events extends LinkedBlockingDeque<Message> {
        @Override public boolean offer(Message event) {
            synchronized (LOCK) {
                offered++;
                published.add(event.getClass().getSimpleName());
            }
            return super.offer(event);
        }
        @Override public Message take() throws InterruptedException {
            synchronized (LOCK) {
                while (granted == taken) LOCK.wait();
                taken++;
            }
            Message event = super.take();
            TAKEN.set(Boolean.TRUE);
            return event;
        }
    }

    static final Object LOCK = new Object();
    static final ThreadLocal<Boolean> TAKEN = new ThreadLocal<>();
    public static final Queue QUEUE = new Queue();
    public static final Events EVENTS = new Events();
    static Thread world;
    static long dispatched, ran, offered, granted, taken, arrived;
    static final List<String> foreign = new ArrayList<>();
    static final List<String> published = new ArrayList<>();

    public static void install() throws ReflectiveOperationException {
        world = Thread.currentThread();
        Pin.install(Class.forName("kotlinx.coroutines.Dispatchers").getDeclaredField("Default"), QUEUE);
        if (kotlinx.coroutines.Dispatchers.getDefault() != QUEUE) throw new IllegalStateException("Dispatchers.Default was not replaced");
        // Initialising ManagementEvents launches its loop, which now waits in QUEUE; its queue is replaced before it first runs.
        Class<?> events = Class.forName("core.worker.ManagementEvents");
        if (QUEUE.blocks.size() != 1) throw new IllegalStateException("management loop was not captured");
        Pin.install(events.getDeclaredField("eventQueue"), EVENTS);
    }

    /**
     * Runs everything handed off so far on the calling (world) thread: queued
     * blocks first-in first-out, then one management event at a time until none
     * is left. Returns [blocks run, events handled].
     */
    public static long[] pump() throws InterruptedException {
        long blocksBefore = ran, eventsBefore = granted;
        while (true) {
            while (true) {
                Runnable block;
                synchronized (LOCK) { block = QUEUE.blocks.poll(); }
                if (block == null) break;
                ran++;
                block.run();
            }
            synchronized (LOCK) {
                if (!QUEUE.blocks.isEmpty()) continue;
                if (offered == granted && arrived == granted) break;
                if (arrived == granted) {
                    granted++;
                    LOCK.notifyAll();
                }
                long deadline = System.nanoTime() + 60_000_000_000L;
                while (arrived != granted) {
                    long left = (deadline - System.nanoTime()) / 1_000_000L;
                    if (left <= 0) throw new IllegalStateException("a management event was published but its handler never resumed");
                    LOCK.wait(left);
                }
            }
        }
        return new long[] { ran - blocksBefore, granted - eventsBefore };
    }

    public static List<String> drainForeign() { synchronized (LOCK) { List<String> copy = new ArrayList<>(foreign); foreign.clear(); return copy; } }
    public static List<String> drainPublished() { synchronized (LOCK) { List<String> copy = new ArrayList<>(published); published.clear(); return copy; } }
}

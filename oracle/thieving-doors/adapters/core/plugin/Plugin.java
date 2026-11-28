package core.plugin;
/** Adapter: the two methods a plugin has. */
public interface Plugin<T> {
    Plugin<T> newInstance(T arg) throws Throwable;
    Object fireEvent(String identifier, Object... args);
}

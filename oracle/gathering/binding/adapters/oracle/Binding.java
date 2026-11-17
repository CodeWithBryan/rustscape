package oracle;
/** Entry point of the binding corpus: the first corpus's driver with the real Skills/Container hooks installed. */
public final class Binding {
    public static void main(String[] args) throws Exception {
        Support.extraReset = Real::reset;
        Support.extraState = Real::state;
        Support.extraSet = Real::set;
        Support.infallible = new java.util.HashSet<>(java.util.Arrays.asList("gameWorld.ticks", "api.getWorldTicks"));
        Oracle.main(args);
    }
}
package core.net;
/** Records decoded requests; does not emulate the original background worker. */
public final class JS5Queue {
    private final IoSession session;
    public JS5Queue(IoSession session) {this.session = session;}
    public void queue(int archive, int group, boolean urgent) {
        session.requests.add(new int[] {archive, group, urgent ? 1 : 0});
    }
}

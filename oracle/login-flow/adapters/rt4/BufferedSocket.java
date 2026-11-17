package rt4;

import java.io.EOFException;
import java.io.IOException;
import java.net.Socket;

/** Scripted stand-in with the original's method set and closed-socket behaviour. */
public final class BufferedSocket {
    final LoginOracle.Wire wire;
    private boolean closed = false;

    public BufferedSocket(Socket socket, SignLink signLink) throws IOException {
        this.wire = ((LoginOracle.ScriptedSocket) socket).wire;
    }
    public void read(int off, int len, byte[] b) throws IOException {
        if (this.closed) return;
        this.wire.check();
        if (this.wire.in.size() < len) throw new EOFException();
        for (int i = 0; i < len; i++) b[off + i] = (byte) (int) this.wire.in.poll();
    }
    public int read() throws IOException {
        return this.closed ? 0 : LoginOracle.blockingRead(this.wire);
    }
    public void write(byte[] src, int len) throws IOException {
        if (this.closed) return;
        this.wire.check();
        LoginOracle.out.println("write " + LoginOracle.hex(src, len));
    }
    public int available() throws IOException {
        if (this.closed) return 0;
        this.wire.check();
        return this.wire.in.size();
    }
    public void close() {
        if (this.closed) return;
        this.closed = true;
        LoginOracle.out.println("close");
    }
}
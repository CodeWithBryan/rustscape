package rt4;
import java.io.*;
import java.net.Socket;
/** Real socket I/O for the unmodified Js5NetQueue; not a BufferedSocket implementation oracle. */
public final class BufferedSocket {
    private final Socket socket;
    private final DataInputStream input;
    private final OutputStream output;
    public BufferedSocket(Socket socket) throws IOException {
        this.socket = socket; socket.setSoTimeout(3000); socket.setTcpNoDelay(true);
        input = new DataInputStream(socket.getInputStream()); output = socket.getOutputStream();
    }
    public int available() throws IOException {return input.available();}
    public void read(int offset, int length, byte[] bytes) throws IOException {input.readFully(bytes, offset, length);}
    public void write(byte[] bytes, int length) throws IOException {output.write(bytes, 0, length); output.flush();}
    public void checkError() throws IOException {if (socket.isClosed()) throw new IOException("closed");}
    public void close() {try {socket.close();} catch (IOException ignored) {}}
    public void breakConnection() {close();}
}

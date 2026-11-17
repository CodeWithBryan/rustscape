package rt4;

/** Oracle adapter: the outgoing buffer and socket calls Cheat makes, recorded. */
public class Protocol {
	public static final Out outboundBuffer = new Out();
	public static final Socket socket = new Socket();

	public static void handleConnectionLost() {
		OracleLog.add("handleConnectionLost");
	}

	public static final class Out {
		public void p1isaac(int opcode) {
			OracleLog.add("p1isaac " + opcode);
		}

		public void p1(int value) {
			OracleLog.add("p1 " + value);
		}

		public void pjstr(JagString value) {
			OracleLog.add("pjstr " + value.toString());
		}
	}

	public static final class Socket {
		public void breakConnection() {
			OracleLog.add("socket.breakConnection");
		}
	}
}
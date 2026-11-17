package core.net.packet;

/** Instantiates the writer and sends the context, as PacketRepository.send does for these writers. */
public class PacketRepository {
	public static void send(Class cls, Context c) {
		OutgoingPacket p;
		try {
			p = (OutgoingPacket) cls.getDeclaredConstructor().newInstance();
		} catch (Exception e) {
			throw new RuntimeException(e);
		}
		p.send(c);
	}
}

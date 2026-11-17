package core.cache.crypto;

/** The pinned profile's disabled ISAAC: every opcode word is zero. */
public class ISAACCipher {
	public int getNextValue() {
		return 0;
	}
}

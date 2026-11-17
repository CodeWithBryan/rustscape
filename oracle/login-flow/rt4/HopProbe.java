package rt4;

/** Finds Math.random() values for which the original hop request fits its 128-byte buffer. */
public final class HopProbe {
    public static void main(String[] args) {
        JagString user = JagString.decodeString("user".getBytes(), 4, 0);
        JagString pass = JagString.decodeString("pass".getBytes(), 4, 0);
        int found = 0;
        for (int k = 0; k < 100000 && found < 3; k++) {
            double d2 = (k + 0.5) / 100000.0;
            Buffer hopBuffer = new Buffer(128);
            hopBuffer.p1(10);
            hopBuffer.p2((int) (0.5 * 99999.0D));
            hopBuffer.p2(530);
            hopBuffer.p8(user.encode37());
            hopBuffer.p4((int) (d2 * 9.9999999E7D));
            hopBuffer.pjstr(pass);
            hopBuffer.p4((int) (0.75 * 9.9999999E7D));
            try {
                hopBuffer.rsaenc(GlobalConfig.RSA_EXPONENT, GlobalConfig.RSA_MODULUS);
                System.out.println(d2 + " " + hopBuffer.offset);
                found++;
            } catch (RuntimeException ex) {
            }
        }
    }
}

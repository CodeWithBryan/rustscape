import rt4.*;

import java.io.*;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Random;

/**
 * Deterministic scenario generator + recorder for the unchanged rt4
 * SoundPcmStream / PcmSound / PcmResampler.
 *
 * usage: StreamProbe <seed> <streamScenarios> <resamplerCases> <outFile>
 *
 * Line kinds (space separated tokens):
 *   S hex rate start end pp outRate stereo link ctor a b c  (NULL | EXC | OK state) { | op args > (EXC | state [buf]) }
 *   P g src dest hex rate pos > ctor(EXC|OK) resample(EXC|hex) scaleRate(EXC|n) scalePosition(EXC|n)
 *   Q g src dest hex rate start end pp > (EXC | hex rate start end)
 * state = linked,pos,loopCount,getVolume,getPan,getAbsoluteRate,rawRate,rawVolume,rawPan,cur,left,right,
 *         vd,lvd,rvd,fade,isTransitioning,isOutOfBounds,getEffectiveVolume,getActiveChannelCount
 */
public class StreamProbe {
	static final String[] PRIV = {"loopCount", "fadeRemaining", "sampleRate", "volume", "pan"};
	static Field fLoop, fFade, fRate, fVol, fPan;
	static volatile String current = "";
	static volatile long lastProgress = System.currentTimeMillis();

	static Field priv(String n) throws Exception {
		Field f = SoundPcmStream.class.getDeclaredField(n);
		f.setAccessible(true);
		return f;
	}

	public static void main(String[] a) throws Exception {
		long seed = Long.parseLong(a[0]);
		int streams = Integer.parseInt(a[1]);
		int resamplers = Integer.parseInt(a[2]);
		fLoop = priv("loopCount");
		fFade = priv("fadeRemaining");
		fRate = priv("sampleRate");
		fVol = priv("volume");
		fPan = priv("pan");
		Thread dog = new Thread(() -> {
			while (true) {
				try {
					Thread.sleep(2000);
				} catch (InterruptedException e) {
					return;
				}
				if (System.currentTimeMillis() - lastProgress > 30000) {
					System.err.println("HANG in: " + current);
					Runtime.getRuntime().halt(3);
				}
			}
		});
		dog.setDaemon(true);
		dog.start();
		Random r = new Random(seed);
		if (a.length > 4) {
			excLog = new PrintWriter(new FileWriter(a[4]));
		}
		long steps = 0, exc = 0, nul = 0;
		try (Writer w = new BufferedWriter(new OutputStreamWriter(new FileOutputStream(a[3]), StandardCharsets.US_ASCII), 1 << 20)) {
			for (int i = 0; i < resamplers; i++) {
				StringBuilder sb = new StringBuilder();
				if (r.nextInt(4) == 0) {
					soundResample(r, sb);
				} else {
					resampler(r, sb);
				}
				current = sb.toString();
				lastProgress = System.currentTimeMillis();
				w.write(sb.toString());
				w.write('\n');
			}
			for (int i = 0; i < streams; i++) {
				StringBuilder sb = new StringBuilder();
				long[] counts = new long[3];
				scenario(r, sb, counts);
				steps += counts[0];
				exc += counts[1];
				nul += counts[2];
				lastProgress = System.currentTimeMillis();
				w.write(sb.toString());
				w.write('\n');
			}
		}
		if (excLog != null) {
			excLog.close();
		}
		System.out.println("resampler cases " + resamplers + ", stream scenarios " + streams + ", step observations " + steps
			+ ", scenarios ending in a Java exception " + exc + ", null creations " + nul);
	}

	// ---- shared helpers -------------------------------------------------

	static String hex(byte[] b) {
		if (b.length == 0) {
			return "-";
		}
		StringBuilder sb = new StringBuilder(b.length * 2);
		for (byte x : b) {
			sb.append(Character.forDigit((x >> 4) & 15, 16)).append(Character.forDigit(x & 15, 16));
		}
		return sb.toString();
	}

	static String big(byte[] b) throws Exception {
		if (b.length <= 48) {
			return hex(b);
		}
		return "h:" + b.length + ":" + hex(MessageDigest.getInstance("SHA-256").digest(b));
	}

	static int capLen(int len, int src, int dest) {
		while ((long) len * Math.abs((long) dest) / Math.max(1L, Math.abs((long) src)) > 200000L) {
			len /= 2;
		}
		return len;
	}

	static int pick(Random r, int... v) {
		return v[r.nextInt(v.length)];
	}

	static int range(Random r, int lo, int hi) {
		return lo + r.nextInt(hi - lo + 1);
	}

	static byte[] samples(Random r, int len) {
		byte[] b = new byte[len];
		int mode = r.nextInt(10);
		if (mode == 0) {
			for (int i = 0; i < len; i++) {
				b[i] = (byte) (r.nextBoolean() ? 127 : -128);
			}
		} else if (mode == 1) {
			for (int i = 0; i < len; i++) {
				b[i] = (byte) (i * 7 - 100);
			}
		} else {
			r.nextBytes(b);
		}
		return b;
	}

	// ---- resampler ------------------------------------------------------

	static final int[] RATES = {22050, 44100, 11025, 8000, 48000, 16000, 32000, 22050, 44100, 96000, 5512, 1, 2, 3};

	static int resRate(Random r) {
		int k = r.nextInt(100);
		if (k < 70) {
			return RATES[r.nextInt(RATES.length)];
		}
		if (k < 90) {
			return range(r, 1, 60000);
		}
		if (k < 96) {
			return range(r, 1, 300);
		}
		if (k < 98) {
			return 0;
		}
		return -range(r, 1, 30000);
	}

	static int globalRate(Random r) {
		return r.nextInt(8) == 0 ? pick(r, 44100, 11025, 8000, 48000, 0, 1) : 22050;
	}

	static final int[][] FIXED_PAIRS = {{22050, 44100}, {44100, 22050}, {8000, 22050}, {11025, 22050}, {22050, 22050}, {48000, 22050},
		{22050, 11025}, {22050, 48000}, {22050, 8000}, {22050, 96000}, {22050, 32000}, {22050, 16000}, {8000, 44100}, {16000, 44100}};
	static int fixedIndex = 0;

	static void resampler(Random r, StringBuilder sb) throws Exception {
		int g = globalRate(r);
		int src, dest;
		if (fixedIndex < FIXED_PAIRS.length * 4) {
			src = FIXED_PAIRS[fixedIndex % FIXED_PAIRS.length][0];
			dest = FIXED_PAIRS[fixedIndex % FIXED_PAIRS.length][1];
			g = 22050;
			fixedIndex++;
		} else {
			src = resRate(r);
			dest = resRate(r);
		}
		int len = pick(r, 0, 1, 2, 13, 14, 15, range(r, 0, 60), range(r, 0, 60), range(r, 0, 400), range(r, 0, 2000));
		len = capLen(len, src, dest);
		byte[] in = samples(r, len);
		int rate = pick(r, 22050, 0, 1, -1, r.nextInt(), range(r, 0, 100000));
		int pos = pick(r, 0, 1, -1, len, r.nextInt(), range(r, 0, 5000));
		sb.append("P ").append(g).append(' ').append(src).append(' ').append(dest).append(' ').append(hex(in)).append(' ').append(rate).append(' ')
			.append(pos).append(" >");
		GlobalConfig.AUDIO_SAMPLE_RATE = g;
		PcmResampler p;
		try {
			p = new PcmResampler(src, dest);
		} catch (Throwable t) {
			sb.append(" EXC");
			GlobalConfig.AUDIO_SAMPLE_RATE = 22050;
			return;
		}
		GlobalConfig.AUDIO_SAMPLE_RATE = 22050;
		sb.append(" OK");
		try {
			String v = big(p.resample(in));
			sb.append(' ').append(v);
		} catch (Throwable t) {
			sb.append(" EXC");
		}
		try {
			int v = p.scaleRate(rate);
			sb.append(' ').append(v);
		} catch (Throwable t) {
			sb.append(" EXC");
		}
		try {
			int v = p.scalePosition(pos);
			sb.append(' ').append(v);
		} catch (Throwable t) {
			sb.append(" EXC");
		}
	}

	static void soundResample(Random r, StringBuilder sb) throws Exception {
		int g = 22050;
		int src = resRate(r), dest = resRate(r);
		int len = capLen(range(r, 0, 200), src, dest);
		byte[] in = samples(r, len);
		int rate = pick(r, 22050, 11025, 44100, range(r, 0, 100000));
		int start, end;
		int k = r.nextInt(10);
		if (k < 3) {
			start = end = range(r, 0, len);
		} else if (k < 8) {
			start = range(r, 0, len);
			end = range(r, start, len);
		} else {
			start = range(r, -3, len + 3);
			end = range(r, -3, len + 3);
		}
		boolean pp = r.nextBoolean();
		sb.append("Q ").append(g).append(' ').append(src).append(' ').append(dest).append(' ').append(hex(in)).append(' ').append(rate).append(' ')
			.append(start).append(' ').append(end).append(' ').append(pp ? 1 : 0).append(" >");
		try {
			PcmResampler p = new PcmResampler(src, dest);
			PcmSound s = new PcmSound(rate, in, start, end, pp);
			PcmSound out = s.resample(p);
			sb.append(' ').append(big(out.samples)).append(' ').append(out.rate).append(' ').append(out.start).append(' ').append(out.end);
		} catch (Throwable t) {
			sb.append(" EXC");
		}
	}

	// ---- stream ---------------------------------------------------------

	static int rate248(Random r) {
		int k = r.nextInt(100);
		if (flavor == 1 && r.nextInt(100) < 85) {
			return r.nextInt(6) == 0 ? -256 : 256;
		}
		if (k < 35) {
			return 256;
		}
		if (k < 55) {
			return pick(r, 128, 512, 64, 1024, 255, 257, 300, 77, 181);
		}
		if (k < 80) {
			return range(r, 1, 2000);
		}
		if (k < 84) {
			return 1;
		}
		if (k < 88) {
			return 0;
		}
		if (k < 92) {
			return pick(r, 65536, 1 << 20, 30000, 4096);
		}
		if (k < 96) {
			return -pick(r, 256, 128, 512, 300, 1, range(r, 1, 2000));
		}
		if (k < 98) {
			return pick(r, Integer.MAX_VALUE, Integer.MIN_VALUE, Integer.MAX_VALUE - 255, 1 << 30);
		}
		return r.nextInt();
	}

	static int rawVolume(Random r) {
		int k = r.nextInt(100);
		if (k < 10) {
			return 0;
		}
		if (k < 55) {
			return range(r, 1, 16384);
		}
		if (k < 80) {
			return range(r, 0, 255) << 6;
		}
		if (k < 86) {
			return -range(r, 1, 5000);
		}
		if (k < 95) {
			return range(r, 16385, 1 << 20);
		}
		if (k < 98) {
			return pick(r, Integer.MAX_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE + 1, 1 << 30);
		}
		return r.nextInt();
	}

	static int pan(Random r) {
		int k = r.nextInt(100);
		if (k < 30) {
			return 8192;
		}
		if (k < 40) {
			return pick(r, 0, 16384);
		}
		if (k < 85) {
			return range(r, 0, 16384);
		}
		if (k < 92) {
			return pick(r, -1, -100, Integer.MIN_VALUE);
		}
		return pick(r, 16385, 20000, Integer.MAX_VALUE, r.nextInt());
	}

	static int duration(Random r) {
		int k = r.nextInt(100);
		if (k < 10) {
			return 0;
		}
		if (k < 20) {
			return 1;
		}
		if (k < 60) {
			return range(r, 2, 50);
		}
		if (k < 88) {
			return range(r, 51, 2000);
		}
		if (k < 95) {
			return pick(r, 100000, Integer.MAX_VALUE, 1 << 20);
		}
		return pick(r, -1, -10, Integer.MIN_VALUE, -range(r, 1, 300));
	}

	static int lcg(int x) {
		return x * 1103515245 + 12345;
	}

	static final int[] EXTREMES = {Integer.MIN_VALUE, Integer.MAX_VALUE, -1, 0, 1, 0x7fff0000, Integer.MIN_VALUE + 1, 0x80008000};

	static void fill(int[] buf, int mode, int seed) {
		int x = seed;
		for (int i = 0; i < buf.length; i++) {
			x = lcg(x);
			if (mode == 0) {
				buf[i] = 0;
			} else if (mode == 1) {
				buf[i] = x ^ (x >>> 13);
			} else if (mode == 2) {
				buf[i] = x >> 14;
			} else {
				buf[i] = EXTREMES[(x >>> 9) % EXTREMES.length];
			}
		}
	}

	static String bufToken(int[] buf) throws Exception {
		if (buf.length <= 10) {
			StringBuilder sb = new StringBuilder("v");
			for (int v : buf) {
				sb.append(':').append(v);
			}
			return sb.toString();
		}
		MessageDigest md = MessageDigest.getInstance("SHA-256");
		byte[] b = new byte[buf.length * 4];
		for (int i = 0; i < buf.length; i++) {
			int v = buf[i];
			b[i * 4] = (byte) (v >> 24);
			b[i * 4 + 1] = (byte) (v >> 16);
			b[i * 4 + 2] = (byte) (v >> 8);
			b[i * 4 + 3] = (byte) v;
		}
		return "h:" + hex(md.digest(b));
	}

	static void state(SoundPcmStream s, StringBuilder sb) throws Exception {
		sb.append(s.isLinked() ? 1 : 0).append(',').append(s.samplePosition).append(',').append(fLoop.getInt(s)).append(',').append(s.getVolume())
			.append(',').append(s.getPan()).append(',').append(s.getAbsoluteRate()).append(',').append(fRate.getInt(s)).append(',')
			.append(fVol.getInt(s)).append(',').append(fPan.getInt(s)).append(',').append(s.currentVolume).append(',').append(s.leftVolume)
			.append(',').append(s.rightVolume).append(',').append(s.volumeDelta).append(',').append(s.leftVolumeDelta).append(',')
			.append(s.rightVolumeDelta).append(',').append(fFade.getInt(s)).append(',').append(s.isTransitioning() ? 1 : 0).append(',')
			.append(s.isOutOfBounds() ? 1 : 0).append(',').append(s.getEffectiveVolume()).append(',').append(s.getActiveChannelCount());
	}

	static PrintWriter excLog;
	static int flavor;

	static void logExc(Throwable t, int len, StringBuilder sb) {
		if (excLog == null) {
			return;
		}
		StackTraceElement[] st = t.getStackTrace();
		String at = st.length == 0 ? "?" : st[0].getMethodName() + ":" + st[0].getLineNumber();
		String caller = st.length < 2 ? "?" : st[1].getMethodName() + ":" + st[1].getLineNumber();
		String line = sb.toString();
		excLog.println(t.getClass().getSimpleName() + " (" + t.getMessage() + ") at " + at + " <- " + caller + " :: len=" + len
			+ line.substring(line.indexOf(' ', 2)));
	}

	static void link(SoundPcmStream s) {
		Node root = new Node();
		root.nextNode = s;
		root.previousNode = s;
		s.nextNode = root;
		s.previousNode = root;
	}

	static void scenario(Random r, StringBuilder sb, long[] counts) throws Exception {
		// sound; flavor 1 biases towards the whole-sample (rate +-256, aligned position) mixers
		flavor = r.nextInt(100) < 30 ? 1 : 0;
		int k = r.nextInt(100);
		int len = k < 4 ? 0 : k < 14 ? 1 : k < 24 ? range(r, 2, 4) : k < 72 ? range(r, 5, 40) : k < 95 ? range(r, 41, 300) : range(r, 301, 3000);
		byte[] smp = samples(r, len);
		k = r.nextInt(100);
		int soundRate = k < 45 ? 22050 : k < 80 ? pick(r, 11025, 44100, 8000, 16000, 48000) : k < 97 ? range(r, 1, 50000) : pick(r, 0, -22050);
		int start, end;
		k = r.nextInt(100);
		if (len == 0) {
			start = end = 0;
		} else if (k < 18) {
			start = end = range(r, 0, len);
		} else if (k < 72) {
			start = range(r, 0, len - 1);
			end = range(r, start + 1, len);
		} else if (k < 82) {
			start = 0;
			end = len;
		} else if (k < 88 && len > 1) {
			end = range(r, 1, len);
			start = end - 1;
		} else if (k < 93) {
			start = range(r, 0, len);
			end = range(r, 0, start);
		} else {
			start = range(r, -3, len + 3);
			end = range(r, -3, len + 5);
		}
		boolean pp = r.nextInt(100) < 40;
		k = r.nextInt(100);
		int outRate = k < 60 ? 22050 : k < 97 ? pick(r, 44100, 11025, 48000, 8000) : pick(r, 0, -22050, 1);
		boolean stereo = r.nextBoolean();
		boolean linked = r.nextInt(100) < 85;
		int ctor = r.nextInt(100) < 30 ? 0 : 1;
		int a, b = 0, c = 0;
		if (ctor == 0) {
			a = r.nextInt(10) == 0 ? pick(r, -5, 1000, 0, 256, 1 << 26) : range(r, 0, 255);
		} else {
			a = rate248(r);
			b = rawVolume(r);
			c = pan(r);
		}
		sb.append("S ").append(hex(smp)).append(' ').append(soundRate).append(' ').append(start).append(' ').append(end).append(' ')
			.append(pp ? 1 : 0).append(' ').append(outRate).append(' ').append(stereo ? 1 : 0).append(' ').append(linked ? 1 : 0).append(' ')
			.append(ctor).append(' ').append(a).append(' ').append(b).append(' ').append(c);
		current = sb.toString();
		AudioChannel.sampleRate = outRate;
		AudioChannel.stereo = stereo;
		PcmSound sound = new PcmSound(soundRate, smp, start, end, pp);
		SoundPcmStream s;
		try {
			s = ctor == 0 ? SoundPcmStream.create(sound, a) : SoundPcmStream.create(sound, a, b, c);
		} catch (Throwable t) {
			sb.append(" EXC:").append(t.getClass().getSimpleName());
			logExc(t, len, sb);
			counts[1]++;
			return;
		}
		if (s == null) {
			sb.append(" NULL");
			counts[2]++;
			return;
		}
		if (linked) {
			link(s);
		}
		sb.append(" OK ");
		state(s, sb);
		int steps = range(r, 1, 14);
		int channels = stereo ? 2 : 1;
		for (int i = 0; i < steps; i++) {
			StringBuilder op = new StringBuilder(" | ");
			int[] buf = null;
			Runnable action;
			k = r.nextInt(100);
			if (flavor == 1 && r.nextInt(100) < 12) {
				k = 95;
			}
			final SoundPcmStream fs = s;
			if (k < 40) {
				int kk = r.nextInt(100);
				int length = kk < 5 ? 0 : kk < 30 ? range(r, 1, 8) : kk < 75 ? range(r, 9, 64) : range(r, 65, 600);
				int offset = r.nextInt(10) < 6 ? 0 : range(r, 1, 9);
				int slack = r.nextInt(4);
				int mode = r.nextInt(4);
				int seed = r.nextInt();
				buf = new int[(offset + length) * channels + slack];
				fill(buf, mode, seed);
				final int[] fb = buf;
				op.append("R ").append(offset).append(' ').append(length).append(' ').append(slack).append(' ').append(mode).append(' ').append(seed);
				action = () -> fs.read(fb, offset, length);
			} else if (k < 48) {
				int kk = r.nextInt(100);
				int length = kk < 8 ? 0 : kk < 60 ? range(r, 1, 100) : kk < 88 ? range(r, 101, 5000) : kk < 94 ? 1000000
					: pick(r, -1, -100, Integer.MAX_VALUE, Integer.MIN_VALUE, r.nextInt());
				op.append("K ").append(length);
				action = () -> fs.skip(length);
			} else if (k < 60) {
				int kk = r.nextInt(100);
				int loops = kk < 30 ? -1 : kk < 42 ? 0 : kk < 57 ? 1 : kk < 80 ? range(r, 2, 5)
					: pick(r, 1000000, Integer.MAX_VALUE, Integer.MIN_VALUE, -5, 17);
				op.append("LP ").append(loops);
				action = () -> fs.setLoops(loops);
			} else if (k < 64) {
				int v = r.nextInt(8) == 0 ? pick(r, -3, 256, 1 << 25, 1 << 26, Integer.MIN_VALUE, r.nextInt()) : range(r, 0, 255);
				op.append("V ").append(v);
				action = () -> fs.setVolume(v);
			} else if (k < 72) {
				int v = rate248(r);
				op.append("RT ").append(v);
				action = () -> fs.setRate(v);
			} else if (k < 77) {
				int d = duration(r), v = rawVolume(r);
				op.append("FV ").append(d).append(' ').append(v);
				action = () -> fs.fadeToVolume(d, v);
			} else if (k < 84) {
				int d = duration(r), v = rawVolume(r), p = pan(r);
				op.append("FP ").append(d).append(' ').append(v).append(' ').append(p);
				action = () -> fs.fadeToVolumeAndPan(d, v, p);
			} else if (k < 88) {
				int d = duration(r);
				op.append("FO ").append(d);
				action = () -> fs.fadeOutAndRelease(d);
			} else if (k < 93) {
				int kk = r.nextInt(100);
				int p = kk < (flavor == 1 ? 90 : 40) ? range(r, 0, len) << 8 : kk < 85 ? range(r, -600, (len << 8) + 600)
					: pick(r, -1, -2, Integer.MIN_VALUE, Integer.MAX_VALUE, len << 8, (len << 8) - 1, 0);
				op.append("SP ").append(p);
				action = () -> fs.setSamplePosition(p);
			} else if (k < 97) {
				op.append("RV");
				action = fs::reverseDirection;
			} else {
				op.append("LK");
				action = () -> link(fs);
			}
			sb.append(op);
			current = sb.toString();
			counts[0]++;
			try {
				action.run();
			} catch (Throwable t) {
				sb.append(" > EXC:").append(t.getClass().getSimpleName());
				logExc(t, len, sb);
				counts[1]++;
				return;
			}
			sb.append(" > ");
			state(s, sb);
			if (buf != null) {
				sb.append(' ').append(bufToken(buf));
			}
		}
	}
}

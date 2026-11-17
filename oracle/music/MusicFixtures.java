import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import rt4.*;

/**
 * Music observations from the unchanged RT4 classes over a real cache: every
 * song and jingle expanded to MIDI, every instrument decoded, every sound an
 * instrument uses, scripted playback through MidiPlayer, MidiPcmStream and
 * the original AudioChannel block reader, and that reader's voice limiter
 * over seeded sets of voices.
 *
 * Usage: MusicFixtures CACHE_DIRECTORY [dump NAME OUTPUT_FILE]
 */
public final class MusicFixtures {
	private static Js5 songs, jingles, synth, vorbis, instruments;
	private static Method channelRead;

	private static String q(String text) {
		return "\"" + text + "\"";
	}

	private static byte[] instrumentBytes(MidiInstrument instrument) throws Exception {
		Field lookupField = MidiInstrument.class.getDeclaredField("soundLookup");
		lookupField.setAccessible(true);
		int[] lookup = (int[]) lookupField.get(instrument);
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		DataOutputStream out = new DataOutputStream(bytes);
		out.write(instrument.notePan);
		out.write(instrument.noteVolume);
		out.write(instrument.noteExclusiveGroup);
		for (short offset : instrument.notePitchOffset) out.writeShort(offset);
		for (int id : lookup) out.writeInt(id);
		out.writeInt(instrument.volumeMultiplier);
		for (MidiTrackState state : instrument.trackStates) {
			if (state == null) {
				out.writeByte(0);
				continue;
			}
			out.writeByte(1);
			out.writeInt(state.sustainRate);
			out.writeInt(state.vibratoDepth);
			out.writeInt(state.attackRate);
			out.writeInt(state.vibratoDelay);
			out.writeInt(state.releaseRate);
			out.writeInt(state.vibratoSpeed);
			out.writeInt(state.decayRate);
			for (byte[] envelope : new byte[][] {state.attackEnvelope, state.releaseEnvelope}) {
				if (envelope == null) {
					out.writeInt(-1);
				} else {
					out.writeInt(envelope.length);
					out.write(envelope);
				}
			}
		}
		return bytes.toByteArray();
	}

	private static void songCases(int id, Js5 archive) throws Exception {
		for (int group = 0; group < archive.capacity(); group++) {
			Song song = null;
			try {
				song = Song.create(archive, group, 0);
			} catch (RuntimeException absent) {
				// An absent group has no file 0.
			}
			if (song == null) {
				System.out.println("{\"case\":\"song\",\"archive\":" + id + ",\"group\":" + group + ",\"missing\":true}");
			} else {
				System.out.println("{\"case\":\"song\",\"archive\":" + id + ",\"group\":" + group + ",\"length\":"
					+ song.midiBytes.length + ",\"sha256\":" + q(MusicOracle.sha(song.midiBytes)) + "}");
			}
		}
	}

	private static TreeSet<Integer> instrumentCases() throws Exception {
		Field lookupField = MidiInstrument.class.getDeclaredField("soundLookup");
		lookupField.setAccessible(true);
		TreeSet<Integer> sounds = new TreeSet<Integer>();
		for (int id = 0; id < instruments.capacity(); id++) {
			MidiInstrument instrument = null;
			try {
				instrument = MidiInstrument.loadInstrument(instruments, id);
			} catch (RuntimeException absent) {
				// Js5.fetchFile(int) throws for a group that is not a single file.
			}
			if (instrument == null) {
				System.out.println("{\"case\":\"instrument\",\"id\":" + id + ",\"missing\":true}");
				continue;
			}
			for (int sound : (int[]) lookupField.get(instrument)) {
				if (sound != 0) sounds.add(sound - 1);
			}
			System.out.println("{\"case\":\"instrument\",\"id\":" + id + ",\"sha256\":"
				+ q(MusicOracle.sha(instrumentBytes(instrument))) + "}");
		}
		return sounds;
	}

	private static void soundCases(TreeSet<Integer> ids) throws Exception {
		SoundBank bank = new SoundBank(synth, vorbis);
		for (int id : ids) {
			PcmSound sound = null;
			try {
				sound = (id & 1) == 0 ? bank.getSynthSound(id >> 2, null) : bank.getVorbisSound(id >> 2, null);
			} catch (RuntimeException absent) {
				// As above: the archive has no such single-file group.
			}
			if (sound == null) {
				System.out.println("{\"case\":\"sound\",\"id\":" + id + ",\"missing\":true}");
			} else {
				System.out.println("{\"case\":\"sound\",\"id\":" + id + ",\"rate\":" + sound.rate + ",\"start\":" + sound.start
					+ ",\"end\":" + sound.end + ",\"pingPong\":" + sound.pingPongLoop + ",\"length\":" + sound.samples.length
					+ ",\"sha256\":" + q(MusicOracle.sha(sound.samples)) + "}");
			}
		}
	}

	private static Js5 archive(int id) {
		return id == 11 ? jingles : songs;
	}

	/**
	 * Script operations, separated by spaces:
	 * play:ARCHIVE:GROUP:VOLUME (playImmediate), fade:ARCHIVE:GROUP:VOLUME
	 * (playFadeOut with a song), fadeout, stop, volume:N, and run:N. One cycle
	 * of run is MidiPlayer.loop(), tryStartLoadedSong(), then two 256-frame
	 * AudioChannel blocks. Each run reports the hash of everything it read.
	 */
	private static void render(String name, boolean stereo, String script, DataOutputStream dump) throws Exception {
		AudioChannel.init(stereo);
		MidiPcmStream stream = new MidiPcmStream();
		stream.init();
		MidiPlayer.init(stream, instruments, vorbis, synth);
		MidiPlayer.state = 0;
		MidiPlayer.jingle = false;
		MidiPlayer.song = null;
		MidiPlayer.soundBank = null;
		MidiPlayer.songArchive = null;
		AudioChannel channel = new AudioChannel();
		channel.setStream(stream);
		int[] block = new int[stereo ? 512 : 256];
		List<String> runs = new ArrayList<String>();
		int cycle = 0;
		for (String op : script.split(" ")) {
			String[] p = op.split(":");
			if (p[0].equals("play")) {
				MidiPlayer.playImmediate(archive(Integer.parseInt(p[1])), Integer.parseInt(p[2]), Integer.parseInt(p[3]));
			} else if (p[0].equals("fade")) {
				MidiPlayer.playFadeOut(Integer.parseInt(p[2]), archive(Integer.parseInt(p[1])), Integer.parseInt(p[3]));
			} else if (p[0].equals("fadeout")) {
				MidiPlayer.playFadeOut();
			} else if (p[0].equals("stop")) {
				MidiPlayer.stop();
			} else if (p[0].equals("volume")) {
				MidiPlayer.setVolume(Integer.parseInt(p[1]));
			} else if (p[0].equals("run")) {
				MessageDigest digest = MessageDigest.getInstance("SHA-256");
				List<Integer> started = new ArrayList<Integer>();
				int count = Integer.parseInt(p[1]);
				for (int i = 0; i < count; i++, cycle++) {
					MidiPlayer.loop();
					if (MidiPlayer.tryStartLoadedSong()) started.add(cycle);
					for (int n = 0; n < 2; n++) {
						channelRead.invoke(channel, (Object) block);
						byte[] bytes = MusicOracle.words(block, block.length);
						digest.update(bytes);
						if (dump != null) dump.write(bytes);
					}
				}
				runs.add("{\"sha256\":" + q(MusicOracle.hex(digest.digest())) + ",\"started\":" + started
					+ ",\"valid\":" + stream.isValid() + ",\"playing\":" + MidiPlayer.isPlaying()
					+ ",\"state\":" + MidiPlayer.state + ",\"volume\":" + stream.getVolume() + "}");
			} else {
				throw new IllegalArgumentException(op);
			}
		}
		if (dump == null) {
			System.out.println("{\"case\":\"render\",\"name\":" + q(name) + ",\"stereo\":" + stereo + ",\"script\":" + q(script)
				+ ",\"runs\":" + runs + "}");
		}
	}

	/** A silent stream above a mixer, as the note stream sits above its voices. */
	private static final class Parent extends PcmStream {
		private final PcmStream child;

		Parent(PcmStream child) {
			this.child = child;
		}

		public int getActiveChannelCount() {
			return 0;
		}

		public PcmStream firstSubStream() {
			return this.child;
		}

		public PcmStream nextSubStream() {
			return null;
		}

		public void read(int[] buffer, int offset, int length) {
			this.child.read(buffer, offset, length);
		}

		public void skip(int length) {
			this.child.skip(length);
		}
	}

	/**
	 * The priority pass of AudioChannel.read over a mixer of voices, directly
	 * under the channel (parents 1) or under one more silent stream (parents
	 * 2, the shape of the music tree). Each voice is listed in tree order as
	 * [sound, length, loop start, loop end, volume, loops, frames skipped
	 * before the pass]; the result is PcmStream.active of each afterwards.
	 */
	private static void limiterCases() throws Exception {
		java.util.Random random = new java.util.Random(530032L);
		AudioChannel.init(true);
		for (int n = 0; n < 240; n++) {
			int parents = 1 + (n & 1);
			int count = n < 8 ? n * 5 : random.nextInt(96);
			int soundCount = 1 + random.nextInt(n % 3 == 0 ? 4 : 48);
			PcmSound[] sounds = new PcmSound[soundCount];
			for (int i = 0; i < soundCount; i++) {
				int length = 40 + random.nextInt(3000);
				int start = random.nextInt(length - 1);
				int end = start + 1 + random.nextInt(length - start - 1);
				sounds[i] = new PcmSound(22050, new byte[length], start, end);
			}
			int[] loopChoices = {0, 0, 0, -1, 1, 6};
			List<SoundPcmStream> streams = new ArrayList<SoundPcmStream>();
			StringBuilder voices = new StringBuilder();
			for (int i = 0; i < count; i++) {
				int sound = random.nextInt(soundCount);
				int volume = random.nextInt(8) == 0 ? 0 : n % 4 == 1 ? 200 + random.nextInt(56) : random.nextInt(256);
				int loops = loopChoices[random.nextInt(loopChoices.length)];
				int skipped = random.nextInt(4) == 0 ? 0 : random.nextInt(sounds[sound].samples.length - 20);
				SoundPcmStream stream = SoundPcmStream.create(sounds[sound], volume);
				stream.setLoops(loops);
				stream.skip(skipped);
				streams.add(stream);
				voices.append(i == 0 ? "" : ",").append("[" + sound + "," + sounds[sound].samples.length + ","
					+ sounds[sound].start + "," + sounds[sound].end + "," + volume + "," + loops + "," + skipped + "]");
			}
			MixerPcmStream mixer = new MixerPcmStream();
			// addSubStream puts a stream at the head, so add them last first.
			for (int i = count - 1; i >= 0; i--) mixer.addSubStream(streams.get(i));
			AudioChannel channel = new AudioChannel();
			channel.setStream(parents == 1 ? mixer : new Parent(mixer));
			channelRead.invoke(channel, (Object) new int[512]);
			StringBuilder active = new StringBuilder();
			for (int i = 0; i < count; i++) active.append(i == 0 ? "" : ",").append(streams.get(i).active);
			System.out.println("{\"case\":\"limiter\",\"parents\":" + parents + ",\"voices\":[" + voices + "],\"active\":["
				+ active + "]}");
		}
	}

	private static String repeat(String op, int count) {
		StringBuilder out = new StringBuilder();
		for (int i = 0; i < count; i++) out.append(' ').append(op);
		return out.toString();
	}

	/** Name, stereo flag and script of every playback scenario. */
	private static List<String[]> scenarios() {
		List<String[]> list = new ArrayList<String[]>();
		// One second is about 43 cycles. Hashes every 50 cycles localize a difference.
		for (int group : new int[] {0, 2, 35, 62, 76, 98, 127, 151, 163, 286, 327, 412, 500, 601}) {
			list.add(new String[] {"song-" + group, "true", "play:6:" + group + ":255" + repeat("run:50", 16)});
		}
		// The opening of a wide sample of songs.
		for (int group = 5; group < 668; group += 13) {
			list.add(new String[] {"opening-" + group, "true", "play:6:" + group + ":255" + repeat("run:50", 4)});
		}
		list.add(new String[] {"mono-76", "false", "play:6:76:255" + repeat("run:50", 10)});
		list.add(new String[] {"mono-151", "false", "play:6:151:255" + repeat("run:50", 10)});
		for (int group : new int[] {0, 9, 28, 53, 97, 150, 212, 300, 377}) {
			list.add(new String[] {"jingle-" + group, "true", "play:11:" + group + ":255" + repeat("run:50", 14)});
		}
		list.add(new String[] {"quiet-98", "true", "play:6:98:64" + repeat("run:50", 6)});
		list.add(new String[] {"crossfade", "true", "play:6:76:255" + repeat("run:50", 5) + " fade:6:151:255"
			+ repeat("run:50", 12)});
		list.add(new String[] {"fade-to-silence", "true", "play:6:2:255" + repeat("run:50", 4) + " fadeout"
			+ repeat("run:50", 6)});
		list.add(new String[] {"volume-steps", "true", "play:6:163:255" + repeat("run:50", 3) + " volume:128"
			+ repeat("run:50", 3) + " volume:0" + repeat("run:50", 2) + " volume:255" + repeat("run:50", 3)});
		list.add(new String[] {"stop-and-restart", "true", "play:6:35:255" + repeat("run:50", 3) + " stop"
			+ repeat("run:50", 2) + " play:6:35:200" + repeat("run:50", 4)});
		list.add(new String[] {"jingle-over-song", "true", "play:6:127:255" + repeat("run:50", 4) + " play:11:28:255"
			+ repeat("run:50", 8) + " play:6:127:255" + repeat("run:50", 4)});
		list.add(new String[] {"volume-while-loading", "true", "play:6:286:255 volume:100" + repeat("run:50", 5)});
		return list;
	}

	public static void main(String[] args) throws Exception {
		Path directory = Paths.get(args[0]);
		songs = MusicOracle.archive(directory, 6);
		jingles = MusicOracle.archive(directory, 11);
		synth = MusicOracle.archive(directory, 4);
		vorbis = MusicOracle.archive(directory, 14);
		instruments = MusicOracle.archive(directory, 15);
		channelRead = AudioChannel.class.getDeclaredMethod("read", int[].class);
		channelRead.setAccessible(true);
		if (args.length > 1 && args[1].equals("dump")) {
			for (String[] scenario : scenarios()) {
				if (scenario[0].equals(args[2])) {
					DataOutputStream out = new DataOutputStream(new java.io.BufferedOutputStream(
						new java.io.FileOutputStream(args[3])));
					render(scenario[0], Boolean.parseBoolean(scenario[1]), scenario[2], out);
					out.close();
				}
			}
			return;
		}
		AudioChannel.init(true);
		songCases(6, songs);
		songCases(11, jingles);
		soundCases(instrumentCases());
		for (String[] scenario : scenarios()) {
			render(scenario[0], Boolean.parseBoolean(scenario[1]), scenario[2], null);
		}
		limiterCases();
	}
}

import java.io.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import rt4.*;

/**
 * Runs the unchanged RT4 music classes (Song, MidiDecoder, MidiInstrument,
 * SoundBank, VorbisSound, SynthSound, MidiPcmStream, MidiPlayer and the PCM
 * streams) against a real cache directory and prints one JSON object per case.
 * Only rt4.AudioChannel (sample rate and stereo flag) and JagString are
 * adapters. Nothing here opens an audio device.
 */
public final class MusicOracle {
	private static final List<BufferedFile> OPEN = new ArrayList<BufferedFile>();

	static Js5 archive(Path directory, int id) throws Exception {
		BufferedFile data = file(directory.resolve("main_file_cache.dat2"));
		BufferedFile idx255 = file(directory.resolve("main_file_cache.idx255"));
		BufferedFile idx = file(directory.resolve("main_file_cache.idx" + id));
		Cache references = new Cache(255, data, idx255, 16 * 1024 * 1024);
		final Cache storage = new Cache(id, data, idx, 16 * 1024 * 1024);
		byte[] reference = references.read(id);
		final Js5Index index = new Js5Index(reference, Buffer.crc32(reference, reference.length));
		Js5ResourceProvider provider = new Js5ResourceProvider() {
			public Js5Index fetchIndex() {return index;}
			public byte[] fetchGroup(int group) {return storage.read(group);}
			public void prefetchGroup(int group) {}
			public int getPercentageComplete(int group) {return 100;}
		};
		return new Js5(provider, false, false);
	}

	private static BufferedFile file(Path path) throws Exception {
		BufferedFile opened = new BufferedFile(new FileOnDisk(path.toFile(), "r", Long.MAX_VALUE), 4096, 0);
		OPEN.add(opened);
		return opened;
	}

	static String hex(byte[] bytes) {
		StringBuilder out = new StringBuilder();
		for (byte value : bytes) out.append(String.format("%02x", value & 255));
		return out.toString();
	}

	static String sha(byte[] bytes) throws Exception {
		return hex(MessageDigest.getInstance("SHA-256").digest(bytes));
	}

	static byte[] words(int[] values, int count) {
		byte[] out = new byte[count * 4];
		for (int i = 0; i < count; i++) {
			int v = values[i];
			out[i * 4] = (byte) (v >>> 24);
			out[i * 4 + 1] = (byte) (v >>> 16);
			out[i * 4 + 2] = (byte) (v >>> 8);
			out[i * 4 + 3] = (byte) v;
		}
		return out;
	}

	public static void main(String[] args) throws Exception {
		Path directory = Paths.get(args[1]);
		if (args[0].equals("stats")) {
			for (int id : new int[] {4, 6, 11, 14, 15}) {
				Js5 js5 = archive(directory, id);
				System.out.println("archive " + id + " capacity " + js5.capacity());
			}
			Js5 songs = archive(directory, 6);
			Song song = Song.create(songs, Integer.parseInt(args[2]), 0);
			System.out.println("song bytes " + song.midiBytes.length + " " + sha(song.midiBytes));
		} else {
			throw new IllegalArgumentException("unknown MusicOracle mode");
		}
		for (BufferedFile opened : OPEN) opened.close();
	}
}

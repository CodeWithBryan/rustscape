import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import rt4.FractalNoiseTexture;
import rt4.RidgedNoiseTexture;
import rt4.Texture;
import rt4.TextureOpPerlinNoise;

/**
 * The tables behind the HD material renderers, from the unchanged RT4 classes:
 * the ridged and the fractal 64x64x64 noise volumes of
 * MaterialManager.initNoiseTextures, and the 256x64 turbulence table of
 * LiquidMaterialRenderer.generateNoiseTable.
 *
 * LiquidMaterialRenderer itself needs an OpenGL binding to load, so
 * {@link #liquidTable} repeats the body of its static generateNoiseTable
 * over the unchanged TextureOpPerlinNoise and Texture. Nothing here opens a
 * GL context: no texture upload, filtering or fixed-function state is
 * observed.
 *
 * Usage: MaterialsOracle [DUMP_DIRECTORY]
 */
public final class MaterialsOracle {
	private static String hex(byte[] bytes, int from, int to) throws Exception {
		MessageDigest digest = MessageDigest.getInstance("SHA-256");
		digest.update(bytes, from, to - from);
		StringBuilder out = new StringBuilder();
		for (byte b : digest.digest()) {
			out.append(String.format("%02x", b & 0xFF));
		}
		return out.toString();
	}

	/** LiquidMaterialRenderer.generateNoiseTable, statement for statement. */
	private static int[][] liquidTable(float persistence) {
		int[][] table = new int[256][64];
		TextureOpPerlinNoise perlinNoise = new TextureOpPerlinNoise();
		perlinNoise.persistence = (int) (persistence * 4096.0F);
		perlinNoise.octaveCount = 3;
		perlinNoise.frequencyY = 4;
		perlinNoise.normalizeOutput = false;
		perlinNoise.frequencyX = 8;
		perlinNoise.postDecode();
		Texture.setSize(256, 64);
		for (int row = 0; row < 256; row++) {
			perlinNoise.getNoiseRow(row, table[row]);
		}
		return table;
	}

	private static void volume(String name, byte[] data) throws Exception {
		StringBuilder line = new StringBuilder();
		line.append("{\"case\":\"").append(name).append("\",\"bytes\":").append(data.length);
		line.append(",\"sha256\":\"").append(hex(data, 0, data.length)).append("\",\"slices\":[");
		for (int slice = 0; slice < 64; slice++) {
			if (slice > 0) {
				line.append(',');
			}
			line.append('"').append(hex(data, slice * 8192, slice * 8192 + 8192)).append('"');
		}
		// One row through the middle of the volume, as raw texel values.
		line.append("],\"row\":[");
		int start = (32 * 4096 + 32 * 64) * 2;
		for (int i = 0; i < 128; i++) {
			if (i > 0) {
				line.append(',');
			}
			line.append(data[start + i] & 0xFF);
		}
		line.append("]}");
		System.out.println(line);
	}

	private static void table(String name, int[][] table) throws Exception {
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		DataOutputStream out = new DataOutputStream(bytes);
		StringBuilder line = new StringBuilder();
		line.append("{\"case\":\"").append(name).append("\",\"rows\":").append(table.length);
		line.append(",\"columns\":").append(table[0].length).append(",\"values\":[");
		for (int row = 0; row < table.length; row++) {
			for (int col = 0; col < table[row].length; col++) {
				out.writeInt(table[row][col]);
				if (row + col > 0) {
					line.append(',');
				}
				line.append(table[row][col]);
			}
		}
		byte[] raw = bytes.toByteArray();
		line.append("],\"sha256\":\"").append(hex(raw, 0, raw.length)).append("\"}");
		System.out.println(line);
	}

	public static void main(String[] args) throws Exception {
		byte[] ridged = new RidgedNoiseTexture().generateTexture();
		byte[] fractal = new FractalNoiseTexture().generateTexture();
		// LiquidMaterialRenderer's constructor calls it twice, for the S and
		// the T turbulence.
		int[][] liquidX = liquidTable(0.4F);
		int[][] liquidY = liquidTable(0.4F);
		volume("ridged", ridged);
		volume("fractal", fractal);
		table("liquid-x", liquidX);
		table("liquid-y", liquidY);
		if (args.length > 0) {
			Files.createDirectories(Paths.get(args[0]));
			Files.write(Paths.get(args[0], "ridged.bin"), ridged);
			Files.write(Paths.get(args[0], "fractal.bin"), fractal);
		}
	}
}

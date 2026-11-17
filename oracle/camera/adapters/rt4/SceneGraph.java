package rt4;

import org.openrs2.deob.annotation.OriginalArg;
import org.openrs2.deob.annotation.OriginalMember;
import org.openrs2.deob.annotation.Pc;

/**
 * Adapter: the scene arrays Camera reads, with two pieces of the original
 * SceneGraph copied verbatim because the class cannot be compiled alone:
 * getTileHeight (lines 204-223) and the opcode 129 part of readTerrain
 * (lines 5151-5244 and 5299-5312; the tile and environment readers before
 * and between them are left out, so the buffer starts at the first record).
 */
public class SceneGraph {
	public static int[][][] tileHeights = new int[4][105][105];
	public static byte[][][] renderFlags = new byte[4][104][104];
	public static byte[][][] occlusionHeights = new byte[4][104][104];

	@OriginalMember(owner = "client!ql", name = "a", descriptor = "(IIII)I")
	public static int getTileHeight(@OriginalArg(0) int level, @OriginalArg(2) int xFine, @OriginalArg(3) int yFine) {
		if (tileHeights == null) {
			return 0;
		}
		@Pc(12) int x = xFine >> 7;
		@Pc(16) int y = yFine >> 7;
		if (x < 0 || y < 0 || x > 103 || y > 103) {
			return 0;
		}
		@Pc(36) int xFine2 = xFine & 0x7F;
		@Pc(40) int yFine2 = yFine & 0x7F;
		@Pc(42) int virtualLevel = level;
		if (level < 3 && (renderFlags[1][x][y] & 0x2) == 2) {
			virtualLevel = level + 1;
		}
		@Pc(91) int heightZ0 = xFine2 * tileHeights[virtualLevel][x + 1][y + 1] + tileHeights[virtualLevel][x][y + 1] * (128 - xFine2) >> 7;
		@Pc(118) int heightZ1 = xFine2 * tileHeights[virtualLevel][x + 1][y] + (128 - xFine2) * tileHeights[virtualLevel][x][y] >> 7;
		return yFine2 * heightZ0 + (128 - yFine2) * heightZ1 >> 7;
	}

	public static void readTerrainOcclusion(int baseY, int tileX, Buffer buffer) {
		int y;
		int z;
		int count;
		@Pc(146) boolean hasOcclusion = false;
		@Pc(243) int startX;
		@Pc(188) int endX;
		@Pc(190) int startY;
		@Pc(194) int endY;
		while (buffer.offset < buffer.data.length) {
			y = buffer.g1();
			if (y != 129) {
				buffer.offset--;
				break;
			}
			for (z = 0; z < 4; z++) {
				@Pc(168) byte occlusionType = buffer.g1b();
				if (occlusionType == 0) {
					startX = tileX;
					if (tileX < 0) {
						startX = 0;
					} else if (tileX >= 104) {
						startX = 104;
					}
					startY = baseY;
					if (baseY < 0) {
						startY = 0;
					} else if (baseY >= 104) {
						startY = 104;
					}
					endX = tileX + 64;
					endY = baseY + 64;
					if (endY < 0) {
						endY = 0;
					} else if (endY >= 104) {
						endY = 104;
					}
					if (endX < 0) {
						endX = 0;
					} else if (endX >= 104) {
						endX = 104;
					}
					while (startX < endX) {
						while (startY < endY) {
							occlusionHeights[z][startX][startY] = 0;
							startY++;
						}
						startX++;
					}
				} else if (occlusionType == 1) {
					for (startX = 0; startX < 64; startX += 4) {
						for (endX = 0; endX < 64; endX += 4) {
							@Pc(305) byte heightVal = buffer.g1b();
							for (endY = startX + tileX; endY < tileX + startX + 4; endY++) {
								for (@Pc(320) int py = baseY + endX; py < baseY + endX + 4; py++) {
									if (endY >= 0 && endY < 104 && py >= 0 && py < 104) {
										occlusionHeights[z][endY][py] = heightVal;
									}
								}
							}
						}
					}
				} else if (occlusionType == 2 && z > 0) {
					endX = tileX + 64;
					startY = baseY;
					endY = baseY + 64;
					if (endX < 0) {
						endX = 0;
					} else if (endX >= 104) {
						endX = 104;
					}
					if (baseY < 0) {
						startY = 0;
					} else if (baseY >= 104) {
						startY = 104;
					}
					if (endY < 0) {
						endY = 0;
					} else if (endY >= 104) {
						endY = 104;
					}
					startX = tileX;
					if (tileX < 0) {
						startX = 0;
					} else if (tileX >= 104) {
						startX = 104;
					}
					while (endX > startX) {
						while (startY < endY) {
							occlusionHeights[z][startX][startY] = occlusionHeights[z - 1][startX][startY];
							startY++;
						}
						startX++;
					}
				}
			}
			hasOcclusion = true;
		}
		if (hasOcclusion) {
			return;
		}
		for (y = 0; y < 4; y++) {
			for (z = 0; z < 16; z++) {
				for (count = 0; count < 16; count++) {
					startX = (tileX >> 2) + z;
					endX = count + (baseY >> 2);
					if (startX >= 0 && startX < 26 && endX >= 0 && endX < 26) {
						occlusionHeights[y][startX][endX] = 0;
					}
				}
			}
		}
	}
}
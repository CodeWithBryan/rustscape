import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Random;
import java.util.TreeSet;
import rt4.*;

/**
 * Actor observations from the reference client, one JSON object per line.
 *
 * Unchanged classes: BasType (decode), NpcList (processEntity and its
 * phases), PathingEntity (move, teleport, applyBodyLean), SeqType (decode),
 * ProjAnim (setTarget, update, render) and ColorUtils (rgbToHsl).
 * Player.animate and ScriptRunner.clearTileEntityCounts/addPlayersToScene/
 * addNpcsToScene are copied verbatim from the reference into adapter classes
 * when the oracle is generated. Everything else under adapters/ is a stub.
 *
 * Usage: ActorsOracle CACHE_DIRECTORY SEED
 */
public final class ActorsOracle {
	private static final List<BufferedFile> OPEN = new ArrayList<BufferedFile>();
	private static PrintStream out;

	private static Js5 archive(Path directory, int id) throws Exception {
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

	private static String hex(byte[] bytes) {
		StringBuilder text = new StringBuilder();
		for (byte value : bytes) text.append(String.format("%02x", value & 255));
		return text.toString();
	}

	private static String ints(int... values) {
		return Arrays.toString(values).replace(" ", "");
	}

	private static void line(String text) {
		out.print(text);
		out.print('\n');
	}

	private static int pick(Random rng, int... values) {
		return values[rng.nextInt(values.length)];
	}

	private static void u2(ByteArrayOutputStream bytes, int value) {
		bytes.write(value >> 8);
		bytes.write(value);
	}

	private static Object secret(Object target, String name) throws Exception {
		Class<?> type = target.getClass();
		while (true) {
			try {
				Field field = type.getDeclaredField(name);
				field.setAccessible(true);
				return field.get(target);
			} catch (NoSuchFieldException absent) {
				type = type.getSuperclass();
				if (type == null) throw absent;
			}
		}
	}

	private static int secretInt(Object target, String name) throws Exception {
		Object value = secret(target, name);
		return value instanceof Boolean ? ((Boolean) value ? 1 : 0) : (Integer) value;
	}

	// ---- BAS records -------------------------------------------------------

	private static BasType decodeBas(byte[] wire) {
		BasType bas = new BasType();
		bas.decode(new Buffer(wire));
		bas.postDecode();
		return bas;
	}

	private static String basFields(BasType b) {
		return ints(b.idleAnimationId, b.walkAnimation, b.slowWalkAnimationId, b.slowWalkFullTurnAnimationId,
			b.slowWalkCCWTurnAnimationId, b.slowWalkCWTurnAnimationId, b.runAnimationId, b.runFullTurnAnimationId,
			b.runCCWTurnAnimationId, b.runCWTurnAnimationId, b.terrainAlignWidth, b.terrainAlignDepth,
			b.yawAcceleration, b.yawMaxSpeed, b.rollAcceleration, b.rollMaxSpeed, b.rollTargetAngle,
			b.pitchAcceleration, b.pitchMaxSpeed, b.pitchTargetAngle, b.movementAcceleration, b.standingCCWTurn,
			b.standingCWTurn, b.walkFullTurnAnimationId, b.walkCCWTurnAnimationId, b.walkCWTurnAnimationId);
	}

	private static String basTransforms(BasType b) {
		StringBuilder text = new StringBuilder("[");
		if (b.modelRotateTranslate != null) {
			for (int slot = 0; slot < b.modelRotateTranslate.length; slot++) {
				if (b.modelRotateTranslate[slot] == null) continue;
				if (text.length() > 1) text.append(',');
				text.append('[').append(slot).append(',').append(ints(b.modelRotateTranslate[slot])).append(']');
			}
		}
		return text.append(']').toString();
	}

	private static void basCase(int id, byte[] wire) {
		BasType bas = decodeBas(wire);
		line("{\"case\":\"bas\",\"id\":" + id + ",\"hex\":\"" + hex(wire) + "\",\"fields\":" + basFields(bas)
			+ ",\"transforms\":" + basTransforms(bas) + "}");
	}

	/** Every cached BAS record, then seeded streams that use every opcode and some unknown ones. */
	private static List<byte[]> basCases(Path cache, Random rng) throws Exception {
		Js5 configs = archive(cache, 2);
		List<byte[]> accelerated = new ArrayList<byte[]>();
		int capacity = configs.getGroupCapacity(32);
		for (int id = 0; id < capacity; id++) {
			byte[] wire = configs.fetchFile(32, id);
			if (wire == null) continue;
			basCase(id, wire);
			BasType bas = decodeBas(wire);
			if (bas.movementAcceleration != -1 || bas.yawAcceleration != 0 || bas.rollTargetAngle != 0 || bas.pitchTargetAngle != 0) {
				accelerated.add(wire);
			}
		}
		for (int n = 0; n < 400; n++) {
			ByteArrayOutputStream wire = new ByteArrayOutputStream();
			int count = rng.nextInt(24);
			for (int i = 0; i < count; i++) {
				int opcode = pick(rng, 1, 2, 3, 4, 5, 6, 7, 8, 9, 26, 27, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
					10 + rng.nextInt(16), 28, 46 + rng.nextInt(210));
				wire.write(opcode);
				if (opcode == 1) {
					u2(wire, pick(rng, 65535, rng.nextInt(65536)));
					u2(wire, pick(rng, 65535, rng.nextInt(65536)));
				} else if (opcode == 26) {
					wire.write(rng.nextInt(256));
					wire.write(rng.nextInt(256));
				} else if (opcode == 27) {
					wire.write(rng.nextInt(12));
					for (int j = 0; j < 6; j++) u2(wire, rng.nextInt(65536));
				} else if (opcode == 29 || opcode == 31 || opcode == 34 || opcode == 37) {
					wire.write(rng.nextInt(256));
				} else if (opcode >= 2 && opcode <= 9 || opcode == 30 || opcode == 32 || opcode == 33 || opcode == 35 || opcode == 36
					|| opcode >= 38 && opcode <= 45) {
					u2(wire, rng.nextInt(65536));
				}
			}
			wire.write(0);
			basCase(-2 - n, wire.toByteArray());
		}
		return accelerated;
	}

	// ---- Movement, facing and lean ----------------------------------------

	private static byte[] seqWire(Random rng, int id) {
		int n = 2 + rng.nextInt(5);
		ByteArrayOutputStream wire = new ByteArrayOutputStream();
		wire.write(1);
		u2(wire, n);
		for (int i = 0; i < n; i++) u2(wire, 1 + rng.nextInt(6));
		for (int i = 0; i < n; i++) u2(wire, i);
		for (int i = 0; i < n; i++) u2(wire, 1);
		wire.write(2);
		u2(wire, rng.nextInt(n + 2));
		for (int value : new int[] {5, id % 4, 8, 3, 9, (id / 3) % 3, 10, id % 3, 11, 1, 0}) wire.write(value);
		return wire.toByteArray();
	}

	private static SeqType decodeSeq(int id, byte[] wire) {
		SeqType seq = new SeqType();
		seq.id = id;
		seq.decode(new Buffer(wire));
		seq.postDecode();
		return seq;
	}

	private static byte[] basWire(Random rng, int lean) {
		ByteArrayOutputStream wire = new ByteArrayOutputStream();
		wire.write(1);
		u2(wire, 0);
		u2(wire, 1);
		int[][] fixed = {{6, 2}, {38, 3}, {39, 4}, {40, 5}, {41, 6}, {42, 7}};
		for (int[] pair : fixed) {
			wire.write(pair[0]);
			u2(wire, pair[1]);
		}
		if (rng.nextInt(3) == 0) {
			for (int[] pair : new int[][] {{2, 8}, {3, 9}, {4, 10}, {5, 11}}) {
				wire.write(pair[0]);
				u2(wire, pair[1]);
			}
		}
		if (rng.nextInt(3) == 0) {
			for (int[] pair : new int[][] {{7, 12}, {8, 13}, {9, 12}}) {
				wire.write(pair[0]);
				u2(wire, pair[1]);
			}
		}
		if (rng.nextInt(5) != 0) {
			wire.write(37);
			wire.write(pick(rng, 0, 1, 2, 4, 8, 16, 32, 64, 128, 255));
		}
		if (rng.nextInt(4) != 0) {
			wire.write(29);
			wire.write(pick(rng, 1, 2, 3, 5, 8, 16, 40, 255));
			wire.write(30);
			u2(wire, pick(rng, 0, 16, 64, 128, 256, 1024, 4096, 65535));
		}
		if (lean != 0) {
			if (lean == 1 || rng.nextInt(8) != 0) {
				wire.write(31);
				wire.write(pick(rng, 1, 2, 4, 8, 20, 255));
			}
			wire.write(32);
			u2(wire, pick(rng, 0, 8, 32, 64, 300, 65535));
			wire.write(33);
			u2(wire, pick(rng, 0, 64, 200, 512, 4000, 65536 - 128, 65536 - 700));
			if (lean == 1 || rng.nextInt(8) != 0) {
				wire.write(34);
				wire.write(pick(rng, 1, 2, 4, 8, 20, 255));
			}
			wire.write(35);
			u2(wire, pick(rng, 0, 8, 32, 64, 300, 65535));
			wire.write(36);
			u2(wire, pick(rng, 0, 64, 200, 512, 4000, 65536 - 128, 65536 - 700));
		}
		wire.write(0);
		return wire.toByteArray();
	}

	private static void remap(BasType bas) {
		bas.idleAnimationId = 0;
		bas.walkAnimation = 1;
		bas.runAnimationId = 2;
		bas.standingCCWTurn = 3;
		bas.standingCWTurn = 4;
		bas.walkFullTurnAnimationId = 5;
		bas.walkCCWTurnAnimationId = 6;
		bas.walkCWTurnAnimationId = 7;
		bas.slowWalkAnimationId = bas.slowWalkFullTurnAnimationId = bas.slowWalkCCWTurnAnimationId = bas.slowWalkCWTurnAnimationId = -1;
		bas.runFullTurnAnimationId = bas.runCCWTurnAnimationId = bas.runCWTurnAnimationId = -1;
	}

	private static String track(int id, int frame, int next, int clock, int delay, int repeats) {
		return ints(id, frame, next, clock, delay, repeats);
	}

	private static String snapshot(PathingEntity p) {
		List<String> queue = new ArrayList<String>();
		for (int i = p.movementQueueSize - 1; i >= 0; i--) {
			queue.add(ints(p.movementQueueX[i], p.movementQueueY[i], p.movementQueueSpeed[i]));
		}
		return "{\"fine\":" + ints(p.xFine, p.yFine)
			+ ",\"head\":" + ints(p.movementQueueX[0], p.movementQueueY[0], p.movementQueueSpeed[0])
			+ ",\"queue\":" + queue.toString().replace(" ", "")
			+ ",\"catchup\":" + p.movementCatchupTicks
			+ ",\"angles\":" + ints(p.currentAngle, p.targetAngle, p.standingTurnCounter)
			+ ",\"face\":" + ints(p.faceEntity, p.faceX, p.faceY)
			+ ",\"force\":" + ints(p.forceMoveCyclesToStart, p.forceMoveCyclesToDest)
			+ ",\"steps\":" + p.seqMovementSteps
			+ ",\"movement\":" + track(p.movementSeqId, p.movementSeqFrame, p.movementSeqNextFrame, p.movementSeqDelayClock, 0, 0)
			+ ",\"action\":" + track(p.seqId, p.seqFrame, p.seqNextFrame, p.seqDelayClock, p.seqDelay, p.seqReplayCount)
			+ ",\"sounds\":" + SoundPlayer.events.toString().replace(" ", "")
			+ ",\"speed\":" + p.movementSpeed
			+ ",\"yaw\":" + ints(p.yawVelocity, p.preciseYaw, p.preciseTargetYaw, p.yawDistanceTraveled, p.yawDecelThreshold,
				p.yawAccelerating ? 1 : 0) + "}";
	}

	private static String leanSnapshot(PathingEntity p) throws Exception {
		StringBuilder ops = new StringBuilder("[");
		for (int[] op : Model.operations) {
			if (ops.length() > 1) ops.append(',');
			ops.append(ints(op));
		}
		return "{\"lean\":" + ints(secretInt(p, "rollTarget"), secretInt(p, "rollVelocity"), secretInt(p, "rollAngle"),
			secretInt(p, "rollMidpoint"), secretInt(p, "rollAccelerating"), secretInt(p, "pitchTarget"),
			secretInt(p, "pitchVelocity"), secretInt(p, "pitchAngle"), secretInt(p, "pitchMidpoint"),
			secretInt(p, "pitchAccelerating")) + ",\"ops\":" + ops.append(']') + "}";
	}

	private static boolean render(PathingEntity entity, int minimum, List<String> expected) throws Exception {
		Model.minimum = minimum;
		Model.operations.clear();
		try {
			if (entity instanceof Player) {
				((Player) entity).lean(new Model());
			} else {
				((Npc) entity).lean(new Model());
			}
		} catch (ArithmeticException zero) {
			expected.add("{\"throws\":true}");
			return false;
		}
		expected.add(leanSnapshot(entity));
		return true;
	}

	private static void motionCase(Random rng, int number, byte[] real) throws Exception {
		boolean npc = number % 3 == 1;
		boolean rotates = !npc || rng.nextInt(3) != 0;
		int size = npc ? pick(rng, 1, 1, 2, 3) : 1;
		int turn = npc ? pick(rng, 32, 32, 16, 64, 256, 0) : pick(rng, 32, 32, 32, 12);
		int lean = real != null ? 0 : pick(rng, 0, 1, 1, 2);
		byte[] basBytes = real != null ? real : basWire(rng, lean);
		BasType bas = decodeBas(basBytes);
		if (real != null) remap(bas);
		boolean leans = bas.rollTargetAngle != 0 || bas.pitchTargetAngle != 0;
		BasTypeList.types.clear();
		BasTypeList.types.put(1, bas);
		SeqTypeList.types.clear();
		SoundPlayer.events.clear();
		int[] start = {25 + rng.nextInt(50), 25 + rng.nextInt(50)};
		int[] origin = number % 2 == 1 ? new int[] {3152, 3168} : new int[] {0, 0};
		boolean isSelf = !npc && number % 4 != 0;
		List<String> sequences = new ArrayList<String>();
		for (int id = 0; id < 14; id++) {
			byte[] wire = seqWire(rng, id);
			sequences.add("\"" + hex(wire) + "\"");
			SeqTypeList.types.put(id, decodeSeq(id, wire));
		}
		PathingEntity entity;
		Arrays.fill(PlayerList.players, null);
		Arrays.fill(NpcList.npcs, null);
		if (npc) {
			Npc created = new Npc();
			created.type = new NpcType();
			created.type.rotationflag = rotates;
			created.type.size = size;
			entity = created;
			PlayerList.self = null;
			PlayerList.players[2047] = new Player();
		} else {
			Player created = new Player();
			entity = created;
			PlayerList.self = isSelf ? created : null;
			PlayerList.players[2047] = created;
		}
		PlayerList.selfId = 17;
		PlayerList.players[23] = new Player();
		NpcList.npcs[8] = new Npc();
		entity.setSize(size);
		entity.turnSpeed = turn;
		entity.basTypeId = 1;
		Camera.originX = origin[0];
		Camera.originY = origin[1];
		entity.movementQueueX[0] = start[0];
		entity.movementQueueY[0] = start[1];
		entity.xFine = start[0] * 128 + size * 64;
		entity.yFine = start[1] * 128 + size * 64;
		List<String> commands = new ArrayList<String>();
		List<String> expected = new ArrayList<String>();
		int every = 3;
		boolean alive = true;
		for (int tick = 0; tick < 130 && alive; tick++) {
			int cycle = 100 + tick;
			if (tick % 40 == 0) every = pick(rng, 1, 2, 3, 5);
			if (tick % every == 0) {
				int speed = rng.nextInt(3), direction = rng.nextInt(8);
				commands.add("[\"M\"," + speed + "," + direction + "]");
				entity.move(speed, direction);
			}
			if (tick % 17 == 0 && !npc) {
				int id = 8 + rng.nextInt(6), delay = rng.nextInt(4);
				commands.add("[\"A\"," + id + "," + delay + "]");
				Player.animate(delay, id, (Player) entity);
			}
			if (tick % 43 == 9) {
				int x = 20 + rng.nextInt(60), y = 20 + rng.nextInt(60), teleporting = tick % 2 == 0 ? 1 : 0;
				commands.add("[\"P\"," + x + "," + y + "," + teleporting + "]");
				entity.teleport(size, x, y, teleporting != 0);
			}
			if (tick % 47 == 13) {
				int[] v = {20 + rng.nextInt(60), 20 + rng.nextInt(60), 20 + rng.nextInt(60), 20 + rng.nextInt(60), cycle + 3, cycle + 14,
					rng.nextInt(4)};
				commands.add("[\"F\"," + ints(v).substring(1));
				entity.forceMoveStartX = v[0];
				entity.forceMoveStartY = v[1];
				entity.forceMoveDestX = v[2];
				entity.forceMoveDestY = v[3];
				entity.forceMoveCyclesToStart = v[4];
				entity.forceMoveCyclesToDest = v[5];
				entity.forceMoveDirection = v[6];
				entity.movementQueueSize = 1;
				entity.seqMovementSteps = 0;
			}
			if (tick % 19 == 0) {
				int face = pick(rng, -1, 8, 32768 + 23, 32768 + 17);
				commands.add("[\"E\"," + face + "]");
				entity.faceEntity = face;
			}
			if (tick % 23 == 0) {
				int x = (origin[0] + 20 + rng.nextInt(60)) * 2 + rng.nextInt(2), y = (origin[1] + 20 + rng.nextInt(60)) * 2 + rng.nextInt(2);
				commands.add("[\"L\"," + x + "," + y + "]");
				entity.faceX = x;
				entity.faceY = y;
			}
			if (tick % 7 == 0) {
				int[] v = new int[6];
				for (int i = 0; i < 6; i++) v[i] = (20 + rng.nextInt(60)) * 128 + 64;
				commands.add("[\"B\"," + ints(v).substring(1));
				PlayerList.players[23].xFine = v[0];
				PlayerList.players[23].yFine = v[1];
				NpcList.npcs[8].xFine = v[2];
				NpcList.npcs[8].yFine = v[3];
				if (npc) {
					PlayerList.players[2047].xFine = v[4];
					PlayerList.players[2047].yFine = v[5];
				}
			}
			if (tick == 87 || tick == 117) {
				int x = pick(rng, 64, 14000, 1408), y = (30 + rng.nextInt(40)) * 128;
				commands.add("[\"X\"," + x + "," + y + "]");
				entity.xFine = x;
				entity.yFine = y;
			}
			commands.add("[\"T\"," + cycle + "]");
			client.loop = cycle;
			NpcList.processEntity(size, entity);
			expected.add(snapshot(entity));
			SoundPlayer.events.clear();
			for (int frame = leans ? rng.nextInt(3) : 0; frame > 0 && alive; frame--) {
				int minimum = -rng.nextInt(400);
				commands.add("[\"R\"," + minimum + "]");
				alive = render(entity, minimum, expected);
			}
		}
		line("{\"case\":\"motion\",\"id\":" + number + ",\"npc\":" + npc + ",\"rotates\":" + rotates + ",\"size\":" + size
			+ ",\"turn\":" + turn + ",\"bas\":\"" + hex(basBytes) + "\",\"remap\":" + (real != null)
			+ ",\"start\":" + ints(start) + ",\"origin\":" + ints(origin) + ",\"is_self\":" + isSelf
			+ ",\"sequences\":" + sequences.toString().replace(" ", "") + ",\"commands\":" + commands.toString().replace(" ", "")
			+ ",\"expected\":" + expected.toString().replace(" ", "") + "}");
	}

	/** applyBodyLean alone, with the yaw state it reads set directly. */
	private static void leanCase(Random rng, int number) throws Exception {
		byte[] basBytes = basWire(rng, number % 9 == 0 ? 2 : 1);
		BasTypeList.types.clear();
		BasTypeList.types.put(1, decodeBas(basBytes));
		Npc entity = new Npc();
		entity.basTypeId = 1;
		List<String> steps = new ArrayList<String>();
		List<String> expected = new ArrayList<String>();
		int accelerating = 0, velocity = 0;
		boolean alive = true;
		for (int step = 0; step < 200 && alive; step++) {
			if (step % (1 + rng.nextInt(30)) == 0) {
				accelerating = rng.nextInt(3) == 0 ? 0 : 1;
				velocity = pick(rng, 0, 1, -1, 50, -50, 4000, -4000);
			}
			int minimum = -rng.nextInt(400);
			steps.add(ints(accelerating, velocity, minimum));
			entity.yawAccelerating = accelerating != 0;
			entity.yawVelocity = velocity;
			alive = render(entity, minimum, expected);
		}
		line("{\"case\":\"lean\",\"id\":" + number + ",\"bas\":\"" + hex(basBytes) + "\",\"steps\":" + steps.toString().replace(" ", "")
			+ ",\"expected\":" + expected.toString().replace(" ", "") + "}");
	}

	// ---- Scene insertion ---------------------------------------------------

	private static int crowdFine(Random rng, int base, int span, int size) {
		int tile = base + rng.nextInt(span);
		int choice = rng.nextInt(10);
		int sub = choice < 6 ? (size & 1) == 1 ? 64 : 0 : choice < 8 ? pick(rng, 0, 64) : rng.nextInt(128);
		return tile * 128 + sub + (size > 1 && choice < 6 ? (size / 2) * 128 : 0);
	}

	private static void crowdCase(Random rng, int number) {
		Arrays.fill(PlayerList.players, null);
		Arrays.fill(NpcList.npcs, null);
		BasTypeList.types.clear();
		SceneGraph.added.clear();
		int loop = 1000 + rng.nextInt(100);
		client.loop = loop;
		Preferences.manyIdleAnimations = rng.nextBoolean();
		boolean minimap = rng.nextInt(8) == 0;
		int base = pick(rng, 50, 50, 50, 50, -1, 0, 101, 103);
		int span = 1 + rng.nextInt(4);
		int playerCount = pick(rng, 0, 3, 10, 40, 51, 60, 210);
		int npcCount = pick(rng, 0, 5, 30, 100);
		StringBuilder players = new StringBuilder("[");
		TreeSet<Integer> used = new TreeSet<Integer>();
		PlayerList.size = 0;
		Player self = null;
		for (int i = 0; i <= playerCount; i++) {
			boolean own = i == 0;
			int index = own ? 2047 : 1 + rng.nextInt(2046);
			if (!used.add(index)) continue;
			boolean listed = !own || rng.nextInt(10) != 0;
			boolean present = own || rng.nextInt(20) != 0;
			int size = pick(rng, 1, 1, 1, 1, 1, 1, 2, 3);
			int x = crowdFine(rng, base, span, size), y = crowdFine(rng, base, span, size);
			int movement = pick(rng, -1, 0, 5), idle = pick(rng, -1, 0, 5);
			int attached = rng.nextInt(7) == 0 ? 1 : 0, setAt = loop - 5 + rng.nextInt(11), resetAt = loop - 5 + rng.nextInt(11);
			int angle = rng.nextInt(2048), stretches = rng.nextInt(2), visible = rng.nextInt(10) == 0 ? 0 : 1;
			if (present) {
				Player player = new Player();
				player.setSize(size);
				player.xFine = x;
				player.yFine = y;
				player.movementSeqId = movement;
				if (idle == -1) {
					player.basTypeId = -1;
				} else {
					BasType bas = new BasType();
					bas.idleAnimationId = idle;
					BasTypeList.types.put(index, bas);
					player.basTypeId = index;
				}
				player.attachment = attached != 0 ? new Object() : null;
				player.attachmentSetAt = setAt;
				player.attachmentResetAt = resetAt;
				player.currentAngle = angle;
				player.seqStretches = stretches != 0;
				player.visible = visible != 0;
				PlayerList.players[index] = player;
				if (own) self = player;
			}
			if (listed) PlayerList.ids[PlayerList.size++] = index;
			if (players.length() > 1) players.append(',');
			players.append(ints(index, listed ? 1 : 0, present ? 1 : 0, visible, size, x, y, movement, idle, attached, setAt, resetAt, angle,
				stretches));
		}
		PlayerList.self = self;
		StringBuilder npcs = new StringBuilder("[");
		used.clear();
		NpcList.size = 0;
		for (int i = 0; i < npcCount; i++) {
			int index = rng.nextInt(32768);
			if (!used.add(index)) continue;
			int typed = rng.nextInt(20) == 0 ? 0 : 1, top = rng.nextInt(4) == 0 ? 1 : 0, valid = rng.nextInt(10) == 0 ? 0 : 1;
			int interactive = rng.nextInt(5) == 0 ? 0 : 1, size = pick(rng, 1, 1, 1, 1, 2, 3, 5);
			int x = crowdFine(rng, base, span, size), y = crowdFine(rng, base, span, size);
			int angle = rng.nextInt(2048), stretches = rng.nextInt(2);
			Npc npc = new Npc();
			if (typed != 0) {
				npc.type = new NpcType();
				npc.type.toprenderpriority = top != 0;
				npc.type.multiNpcValid = valid != 0;
				npc.type.interactive = interactive != 0;
				npc.type.size = size;
			}
			npc.setSize(size);
			npc.xFine = x;
			npc.yFine = y;
			npc.currentAngle = angle;
			npc.seqStretches = stretches != 0;
			NpcList.npcs[index] = npc;
			NpcList.ids[NpcList.size++] = index;
			if (npcs.length() > 1) npcs.append(',');
			npcs.append(ints(index, typed, top, valid, interactive, size, x, y, angle, stretches));
		}
		int[] flag = rng.nextInt(3) == 0 ? new int[] {self.xFine >> 7, self.yFine >> 7} : new int[] {base + 1, base};
		LoginManager.mapFlagX = flag[0];
		LoginManager.mapFlagY = flag[1];
		ScriptRunner.clearTileEntityCounts();
		if (!minimap) {
			ScriptRunner.addPlayersToScene(true);
			ScriptRunner.addNpcsToScene(true);
			ScriptRunner.addPlayersToScene(false);
		}
		ScriptRunner.addNpcsToScene(false);
		StringBuilder low = new StringBuilder("[");
		for (int i = 0; i < PlayerList.size; i++) {
			Player player = PlayerList.players[PlayerList.ids[i]];
			if (i > 0) low.append(',');
			low.append(player != null && player.lowDetail ? 1 : 0);
		}
		StringBuilder counts = new StringBuilder("[");
		for (int x = 0; x < 104; x++) {
			for (int y = 0; y < 104; y++) {
				if (ScriptRunner.tileEntityCounts[x][y] == 0) continue;
				if (counts.length() > 1) counts.append(',');
				counts.append(ints(x, y, ScriptRunner.tileEntityCounts[x][y]));
			}
		}
		line("{\"case\":\"crowd\",\"id\":" + number + ",\"loop\":" + loop + ",\"minimap\":" + minimap + ",\"flag\":" + ints(flag)
			+ ",\"players\":" + players.append(']') + ",\"npcs\":" + npcs.append(']')
			+ ",\"added\":" + SceneGraph.added.toString().replace(" ", "") + ",\"low\":" + low.append(']')
			+ ",\"self_low\":" + (self.lowDetail ? 1 : 0) + ",\"flag_x\":" + LoginManager.mapFlagX + ",\"counts\":" + counts.append(']')
			+ "}");
	}

	// ---- Projectiles -------------------------------------------------------

	private static void projCase(Random rng, int number) throws Exception {
		SeqTypeList.types.clear();
		SpotAnimTypeList.types.clear();
		SpotAnimType spot = new SpotAnimType();
		String sequence = "null";
		if (rng.nextInt(5) != 0) {
			int n = 1 + rng.nextInt(6);
			ByteArrayOutputStream wire = new ByteArrayOutputStream();
			wire.write(1);
			u2(wire, n);
			// A zero delay is legal; a replay loop made only of zero delays would
			// never leave ProjAnim.update, so those restart at frame 0 instead.
			boolean zeros = rng.nextInt(4) == 0;
			for (int i = 0; i < n; i++) u2(wire, i == 0 ? 1 + rng.nextInt(5) : zeros ? rng.nextInt(3) : 1 + rng.nextInt(6));
			for (int i = 0; i < n; i++) u2(wire, i);
			for (int i = 0; i < n; i++) u2(wire, 1);
			if (rng.nextInt(3) != 0) {
				wire.write(2);
				u2(wire, zeros ? n + rng.nextInt(3) : rng.nextInt(n + 3));
			}
			wire.write(0);
			sequence = "\"" + hex(wire.toByteArray()) + "\"";
			SeqTypeList.types.put(7, decodeSeq(7, wire.toByteArray()));
			spot.seqId = 7;
		}
		SpotAnimTypeList.types.put(3, spot);
		int sx = 128 + rng.nextInt(13000), sy = 128 + rng.nextInt(13000), sz = -rng.nextInt(600);
		int first = 100 + rng.nextInt(40), last = first + rng.nextInt(80);
		int elevation = pick(rng, -1, -1, 0, 16, 32, 64, 100, 254), arc = rng.nextInt(256), target = pick(rng, 0, 5, -3);
		int baseZ = rng.nextInt(64) * 4;
		ProjAnim proj = new ProjAnim(3, 0, sx, sy, sz, first, last, elevation, arc, target, baseZ);
		int tx = rng.nextInt(40) == 0 ? sx : sx - 1500 + rng.nextInt(3000), ty = rng.nextInt(40) == 0 ? sy : sy - 1500 + rng.nextInt(3000);
		int tz = -rng.nextInt(600);
		proj.setTarget(ty, first, tz, tx);
		String initial = ints(tx, ty, tz);
		List<String> steps = new ArrayList<String>();
		List<String> expected = new ArrayList<String>();
		int loop = first - rng.nextInt(4);
		while (true) {
			int delta = rng.nextInt(10) < 7 ? 1 : 2 + rng.nextInt(4);
			loop += delta;
			if (loop > last) break;
			if (loop < first) continue;
			int aimed = target != 0 && rng.nextInt(7) != 0 ? 1 : 0;
			tx += rng.nextInt(33) - 16;
			ty += rng.nextInt(33) - 16;
			tz += rng.nextInt(9) - 4;
			steps.add(ints(loop, delta, aimed, tx, ty, tz));
			if (aimed != 0) proj.setTarget(ty, loop, tz, tx);
			proj.update(delta);
			SpotAnimType.requests.clear();
			Model.operations.clear();
			proj.render(0, 0, 0, 0, 0, 0, 0, 0, -1L, 0, null);
			int[] request = SpotAnimType.requests.get(0);
			int[] rotate = Model.operations.get(0);
			expected.add("{\"at\":" + ints((int) proj.x, (int) proj.y, (int) proj.z, proj.yaw, secretInt(proj, "pitch"),
				secretInt(proj, "seqFrame"), secretInt(proj, "seqNextFrame"), secretInt(proj, "frameCycle"))
				+ ",\"bits\":[\"" + Double.doubleToLongBits(proj.x) + "\",\"" + Double.doubleToLongBits(proj.y) + "\",\""
				+ Double.doubleToLongBits(proj.z) + "\"],\"model\":" + ints(request[0], request[1], request[2], rotate[0], rotate[1]) + "}");
		}
		line("{\"case\":\"proj\",\"id\":" + number + ",\"args\":" + ints(sx, sy, sz, first, last, elevation, arc, target, baseZ)
			+ ",\"sequence\":" + sequence + ",\"initial\":" + initial + ",\"steps\":" + steps.toString().replace(" ", "")
			+ ",\"expected\":" + expected.toString().replace(" ", "") + "}");
	}

	// ---- CS2 5401 colour conversion ---------------------------------------

	private static void hslCases(Random rng) throws Exception {
		MessageDigest digest = MessageDigest.getInstance("SHA-256");
		byte[] block = new byte[2 * 65536];
		for (int high = 0; high < 256; high++) {
			for (int low = 0; low < 65536; low++) {
				int value = ColorUtils.rgbToHsl(high << 16 | low);
				block[low * 2] = (byte) (value >> 8);
				block[low * 2 + 1] = (byte) value;
			}
			digest.update(block);
		}
		StringBuilder samples = new StringBuilder("[");
		for (int i = 0; i < 4000; i++) {
			int rgb = rng.nextInt();
			if (i > 0) samples.append(',');
			samples.append(ints(rgb, ColorUtils.rgbToHsl(rgb), (short) ColorUtils.rgbToHsl(rgb)));
		}
		line("{\"case\":\"hsl\",\"sha256\":\"" + hex(digest.digest()) + "\",\"samples\":" + samples.append(']') + "}");
	}

	public static void main(String[] args) throws Exception {
		out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
		long seed = Long.parseLong(args[1]);
		List<byte[]> accelerated = basCases(Paths.get(args[0]), new Random(seed));
		Random rng = new Random(seed + 1);
		int number = 0;
		for (byte[] real : accelerated) {
			for (int repeat = 0; repeat < 3; repeat++) motionCase(rng, number++, real);
		}
		for (int i = 0; i < 60; i++) motionCase(rng, number++, null);
		rng = new Random(seed + 2);
		for (int i = 0; i < 40; i++) leanCase(rng, i);
		rng = new Random(seed + 3);
		for (int i = 0; i < 200; i++) crowdCase(rng, i);
		rng = new Random(seed + 4);
		for (int i = 0; i < 300; i++) projCase(rng, i);
		hslCases(new Random(seed + 5));
		out.flush();
		for (BufferedFile opened : OPEN) opened.close();
	}
}

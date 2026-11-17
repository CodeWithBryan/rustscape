import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.jetbrains.org.objectweb.asm.ClassReader;
import org.jetbrains.org.objectweb.asm.ClassVisitor;
import org.jetbrains.org.objectweb.asm.ClassWriter;
import org.jetbrains.org.objectweb.asm.MethodVisitor;
import org.jetbrains.org.objectweb.asm.Opcodes;

/**
 * Build-time tool of the social oracle, after oracle/world-trace/tool. The
 * sources are compiled unchanged; this copies their class files and redirects
 * the call sites that read the host's wall clock or construct an unseeded
 * java.util.Random to oracle.Support, which records each read and answers it
 * from the case. Classes under oracle/ (the harness) are copied as they are.
 * A census of every redirected site goes into the fixture's provenance, and a
 * time or identity API this tool does not know stops the build.
 *
 * Usage: Rewrite IN_DIR OUT_DIR CENSUS_JSON
 */
public final class Rewrite {
    static final String PIN = "oracle/Support";
    static final Map<String, Integer> census = new TreeMap<>();
    static final Map<String, Integer> classes = new TreeMap<>();
    static final List<String> unknown = new ArrayList<>();

    static void count(String kind, String owner) {
        census.merge(kind, 1, Integer::sum);
        classes.merge(owner, 1, Integer::sum);
    }

    static final class Method extends MethodVisitor {
        final String owner;
        boolean changed;
        Method(MethodVisitor next, String owner) { super(Opcodes.ASM9, next); this.owner = owner; }

        @Override public void visitMethodInsn(int opcode, String target, String name, String descriptor, boolean isInterface) {
            if (opcode == Opcodes.INVOKESTATIC && target.equals("java/lang/System") && (name.equals("currentTimeMillis") || name.equals("nanoTime"))) {
                count("System." + name, owner);
                super.visitMethodInsn(opcode, PIN, name, descriptor, false);
                return;
            }
            if (opcode == Opcodes.INVOKESPECIAL && name.equals("<init>") && descriptor.equals("()V") && target.equals("java/util/Date")) {
                count("new Date()", owner);
                changed = true;
                super.visitMethodInsn(Opcodes.INVOKESTATIC, PIN, "currentTimeMillis", "()J", false);
                super.visitMethodInsn(opcode, target, name, "(J)V", false);
                return;
            }
            if (opcode == Opcodes.INVOKESPECIAL && name.equals("<init>") && descriptor.equals("()V") && target.equals("java/util/Random")) {
                count("new Random()", owner);
                changed = true;
                super.visitMethodInsn(Opcodes.INVOKESTATIC, PIN, "nextSeed", "()J", false);
                super.visitMethodInsn(opcode, target, name, "(J)V", false);
                return;
            }
            if (opcode == Opcodes.INVOKESTATIC && target.equals("java/util/Calendar") && name.equals("getInstance")) {
                // Util.nextMidnight only; no case calls it.
                count("Calendar.getInstance (not reached)", owner);
            } else if (target.equals("java/security/SecureRandom") && name.equals("<init>")
                || opcode == Opcodes.INVOKESTATIC && name.equals("now") && target.startsWith("java/time/")
                || target.equals("java/time/Clock") || target.equals("java/util/GregorianCalendar") && name.equals("<init>")
                || target.equals("java/util/UUID") && name.equals("randomUUID")
                || target.equals("java/lang/System") && name.equals("identityHashCode")
                || target.equals("java/lang/Math") && name.equals("random")) {
                unknown.add(owner + " calls " + target + "." + name + descriptor);
            }
            super.visitMethodInsn(opcode, target, name, descriptor, isInterface);
        }

        // Each rewritten constructor call pushes one long more than the original.
        @Override public void visitMaxs(int maxStack, int maxLocals) { super.visitMaxs(changed ? maxStack + 2 : maxStack, maxLocals); }
    }

    static byte[] transform(byte[] input) {
        ClassReader reader = new ClassReader(input);
        ClassWriter writer = new ClassWriter(0);
        final String owner = reader.getClassName();
        reader.accept(new ClassVisitor(Opcodes.ASM9, writer) {
            @Override public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                MethodVisitor next = super.visitMethod(access, name, descriptor, signature, exceptions);
                return next == null ? null : new Method(next, owner);
            }
        }, 0);
        return writer.toByteArray();
    }

    static String json(Map<String, Integer> map) {
        return map.entrySet().stream().map(e -> "\"" + e.getKey().replace("\\", "\\\\").replace("\"", "\\\"") + "\":" + e.getValue())
            .collect(Collectors.joining(",", "{", "}"));
    }

    public static void main(String[] args) throws IOException {
        Path in = Paths.get(args[0]), out = Paths.get(args[1]);
        List<Path> files;
        try (Stream<Path> walk = Files.walk(in)) {
            files = walk.filter(Files::isRegularFile).sorted().collect(Collectors.toList());
        }
        int total = 0, rewritten = 0;
        for (Path file : files) {
            String relative = in.relativize(file).toString().replace('\\', '/');
            Path target = out.resolve(relative);
            Files.createDirectories(target.getParent());
            byte[] data = Files.readAllBytes(file);
            total++;
            if (relative.endsWith(".class") && !relative.startsWith("oracle/")) {
                int before = census.values().stream().mapToInt(Integer::intValue).sum();
                byte[] changed = transform(data);
                // A class with no redirected site keeps its compiled bytes exactly.
                if (census.values().stream().mapToInt(Integer::intValue).sum() != before) {
                    data = changed;
                    rewritten++;
                }
            }
            Files.write(target, data);
        }
        if (!unknown.isEmpty()) {
            throw new IllegalStateException("time or identity sources this tool does not pin:\n" + String.join("\n", unknown));
        }
        String report = "{\"classFiles\":" + total + ",\"rewrittenClassFiles\":" + rewritten + ",\"sites\":" + json(census)
            + ",\"classes\":" + json(classes) + "}\n";
        Files.write(Paths.get(args[2]), report.getBytes(StandardCharsets.UTF_8));
    }
}

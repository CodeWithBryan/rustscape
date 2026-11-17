package oracle.boot;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;
import java.util.stream.Stream;

/**
 * Runs every case in a class loader of its own, so the originals' static state (listener maps, tables built in
 * static initialisers) starts fresh for each case. The compiled classes are read once and defined per case; the
 * Kotlin standard library, json-simple and ConstLib come from the parent loader.
 */
public class Boot {
    static final Map<String, byte[]> CLASSES = new HashMap<>();

    static final class CaseLoader extends ClassLoader {
        CaseLoader(ClassLoader parent) { super(parent); }
        @Override protected Class<?> findClass(String name) throws ClassNotFoundException {
            byte[] bytes = CLASSES.get(name);
            if (bytes == null) throw new ClassNotFoundException(name);
            return defineClass(name, bytes, 0, bytes.length);
        }
        @Override protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
            synchronized (getClassLoadingLock(name)) {
                Class<?> c = findLoadedClass(name);
                if (c == null) c = CLASSES.containsKey(name) ? findClass(name) : getParent().loadClass(name);
                if (resolve) resolveClass(c);
                return c;
            }
        }
    }

    public static void main(String[] args) throws Exception {
        Path root = Paths.get(args[0]);
        try (Stream<Path> files = Files.walk(root)) {
            for (Path p : (Iterable<Path>) files.filter(f -> f.toString().endsWith(".class"))::iterator) {
                String name = root.relativize(p).toString().replace('\\', '/').replace('/', '.');
                CLASSES.put(name.substring(0, name.length() - 6), Files.readAllBytes(p));
            }
        }
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                ClassLoader loader = new CaseLoader(Boot.class.getClassLoader());
                Method run = loader.loadClass("oracle.Support").getMethod("runCase", String.class);
                out.println((String) run.invoke(null, line));
            }
        }
        out.flush();
    }
}

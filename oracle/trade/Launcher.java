package oracle;

import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;

/**
 * Runs every case in its own class loader, so TradeModule's companion components,
 * ComponentDefinition's table and every other static start fresh for each case.
 *
 * Arguments: the directory of the oracle classes, then the library jars.
 */
public final class Launcher {
	public static void main(String[] args) throws Exception {
		URL[] libraries = new URL[args.length - 1];
		for (int i = 1; i < args.length; i++) {
			libraries[i - 1] = new File(args[i]).toURI().toURL();
		}
		ClassLoader shared = new URLClassLoader(libraries, ClassLoader.getPlatformClassLoader());
		URL[] classes = {new File(args[0]).toURI().toURL()};
		PrintStream out = new PrintStream(System.out, false, "UTF-8");
		try (BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
			String line;
			while ((line = reader.readLine()) != null) {
				try (URLClassLoader loader = new URLClassLoader(classes, shared)) {
					Class<?> support = loader.loadClass("oracle.Support");
					out.println((String) support.getMethod("run", String.class).invoke(null, line));
				}
			}
		}
		out.flush();
	}
}

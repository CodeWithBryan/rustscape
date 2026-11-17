package rt4;

import java.io.File;

/**
 * Oracle adapter: SignLink.openPreferences without the request thread. The file is the original's
 * "jagex_" + name + "_preferences.dat", opened through the unchanged FileOnDisk in a directory the
 * oracle owns instead of the first existing cache location.
 */
public class SignLink {
	public File directory;
	/** When set, opening fails as the original does when no location is writable (status 2). */
	public boolean unavailable;

	public PrivilegedRequest openPreferences(String cacheSubDir) {
		PrivilegedRequest request = new PrivilegedRequest();
		try {
			if (this.unavailable) {
				throw new java.io.IOException("unavailable");
			}
			request.result = new FileOnDisk(new File(this.directory, "jagex_" + cacheSubDir + "_preferences.dat"), "rw", 10000L);
			request.status = 1;
		} catch (Exception ex) {
			request.status = 2;
		}
		return request;
	}

	public void breakConnection() {
		OracleLog.add("signLink.breakConnection");
	}
}
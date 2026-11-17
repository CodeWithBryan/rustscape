package plugin.api;

/**
 * The plugin hooks of the roof fill with no handler registered, which is the
 * client's state unless a plugin installs one: every hook does nothing and the
 * group limit is the default.
 */
public final class API {
    public static final int TILE_FLAG_UNDER_ROOF = 0x4;

    public static int GetRoofVisibilityGroupLimit() {
        return 2;
    }

    public static boolean IsRoofVisibilityActive() {
        return false;
    }

    public static void DisableRoofVisibilityIfExpired() {
    }

    public static void EnsureRoofVisibilityBuffers() {
    }

    public static void ApplyRoofVisibilityRequests() {
    }

    public static void BeginRoofVisibilityGroup(int group) {
    }

    public static void AddRoofVisibilityGroupTile(int group, int plane, int sceneX, int sceneY) {
    }
}

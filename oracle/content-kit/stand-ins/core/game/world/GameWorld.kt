// Replaces the dialogue-dsl Java GameWorld stand-in (same calls for Java callers: getSettings(), getName()) with an
// object, so content's `GameWorld.settings?.name` compiles as it does against the original's Kotlin object.
package core.game.world
import oracle.Rec
object GameWorld {
    class Settings(val name: String)
    @JvmStatic val settings: Settings?
        get() { Rec.emit("settings"); return oracle.Oracle.settings() }
}

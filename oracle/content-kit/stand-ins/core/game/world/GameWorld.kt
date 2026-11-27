// Replaces the dialogue-dsl Java GameWorld stand-in (same calls for Java callers: getSettings(), getName()) with an
// object, so content's `GameWorld.settings?.name` compiles as it does against the original's Kotlin object.
// The read is recorded by caller: content's own read (a class of the `content.` package) as "GameWorld.settings",
// the dialogue core's (DialogueInterpreter.doSubstitutions' @servername) as "settings", the name dialogue-dsl
// records. A corpus can then require the content's read and tell the two apart in a failure sweep.
package core.game.world
import oracle.Rec
object GameWorld {
    class Settings(val name: String)
    @JvmStatic val settings: Settings?
        get() {
            val caller = Thread.currentThread().stackTrace.drop(1)
                .first { !it.className.startsWith("core.game.world.GameWorld") }.className
            Rec.emit(if (caller.startsWith("content.")) "GameWorld.settings" else "settings")
            return oracle.Oracle.settings()
        }
}

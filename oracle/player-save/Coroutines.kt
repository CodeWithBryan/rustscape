// Oracle-only stand-in for the three kotlinx.coroutines symbols PlayerSaver.kt
// imports. The pinned compiler artifacts do not include kotlinx-coroutines, so
// the unmodified saver is compiled against this adapter. Blocks run on the
// calling thread and must finish without suspending; dispatcher selection and
// real coroutine scheduling are therefore not part of this corpus.
package kotlinx.coroutines

import kotlin.coroutines.Continuation
import kotlin.coroutines.CoroutineContext
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

interface CoroutineScope

private object Scope : CoroutineScope

object Dispatchers {
    @JvmField
    val IO: CoroutineContext = EmptyCoroutineContext
}

fun <T> runBlocking(context: CoroutineContext = EmptyCoroutineContext, block: suspend CoroutineScope.() -> T): T {
    var outcome: Result<T>? = null
    block.startCoroutine(Scope, Continuation(context) { outcome = it })
    val finished = outcome ?: throw IllegalStateException("oracle coroutine adapter cannot suspend")
    return finished.getOrThrow()
}

@Suppress("UNUSED_PARAMETER", "RedundantSuspendModifier")
suspend fun <T> withContext(context: CoroutineContext, block: suspend CoroutineScope.() -> T): T = Scope.block()

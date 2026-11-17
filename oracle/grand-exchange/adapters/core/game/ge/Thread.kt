package core.game.ge

/**
 * `Thread { ... }.start()` of GrandExchange.boot: the body is kept and run by
 * the case's "worker" action, on the case thread, until the queue is empty.
 */
class Thread(private val body: () -> Unit) {
    var name: String = ""

    fun start() {
        oracle.Support.startWorker(body)
    }

    companion object {
        private val current = Thread {}

        @JvmStatic
        fun currentThread(): Thread = current
    }
}
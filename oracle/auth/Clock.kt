package core.auth
/** Explicit wall-clock adapter; original source calls retain their exact text. */
object System {var now=0L;fun currentTimeMillis()=now+core.game.system.SystemManager.elapsedMillis}

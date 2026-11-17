package core

/** Only the path fields the compiled drop-table classes read; declarations mirror the original companion. */
class ServerConstants {
    companion object {
        var CONFIG_PATH: String? = null

        @JvmField
        var RDT_DATA_PATH: String? = null

        @JvmField
        var CELEDT_DATA_PATH: String? = null

        @JvmField
        var USDT_DATA_PATH: String? = null

        @JvmField
        var HDT_DATA_PATH: String? = null

        @JvmField
        var GDT_DATA_PATH: String? = null

        @JvmField
        var RSDT_DATA_PATH: String? = null

        @JvmField
        var ASDT_DATA_PATH: String? = null
    }
}

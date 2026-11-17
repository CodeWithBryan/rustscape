import core.net.packet.`in`.Login
import core.net.packet.`in`.LoginInfo
import core.game.world.repository.Repository
import core.tools.StringUtils
import core.cache.crypto.ISAACCipher
import core.cache.misc.buffer.ByteBufferUtils
import core.ServerConstants
import rt4.WriterSupport
import rt4.GlobalConfig
import java.nio.ByteBuffer
import java.util.Random

private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it.toInt() and 255) }
private fun json(value: Any?): String = when(value) {
    null -> "null"
    is String -> "\"" + value.map { c -> when(c) {
        '\\' -> "\\\\"; '"' -> "\\\""; else -> if(c.code < 32 || c.code > 126) "\\u%04x".format(c.code) else c.toString()
    }}.joinToString("") + "\""
    is ByteArray -> json(hex(value))
    is IntArray -> value.joinToString(",", "[", "]")
    is Map<*,*> -> value.entries.joinToString(",", "{", "}") { json(it.key.toString())+":"+json(it.value) }
    is Iterable<*> -> value.joinToString(",", "[", "]") { json(it) }
    else -> value.toString()
}
private fun emit(vararg fields: Pair<String, Any?>) { println(json(linkedMapOf(*fields))) }
private fun cipher(cipher: ISAACCipher): IntArray {
    val field=ISAACCipher::class.java.getDeclaredField("results");field.isAccessible=true
    val values=field.get(cipher) as IntArray
    return IntArray(8) { values[255-it] }
}
private fun info(info: LoginInfo?): Any? = info?.let {
    linkedMapOf("opcode" to it.opcode, "showAds" to it.showAds,"windowMode" to it.windowMode,
        "width" to it.screenWidth,"height" to it.screenHeight,"displayMode" to it.displayMode,
        "affiliate" to it.adAffiliateId,"settingsHash" to it.settingsHash,"packetCount" to it.currentPacketCount,
        "crcs" to it.crcSums,"username" to it.username,"password" to it.password,
        "incomingInitialKeys" to cipher(it.isaacPair.input),"outgoingInitialKeys" to cipher(it.isaacPair.output),
        "incomingDisabled" to IntArray(520) { _ -> it.isaacPair.input.nextValue }.distinct(),
        "outgoingDisabled" to IntArray(520) { _ -> it.isaacPair.output.nextValue }.distinct())
}
private fun decoded(packet: ByteArray, online: Boolean): Map<String,Any?> {
    Repository.online.clear();Repository.lookups.clear()
    // The supplied name set is broad only for these fixtures, to exercise lookup ordering.
    if(online) Repository.online.addAll(listOf("alice","agent_42","bob","","zzzzzzzzzzzz","a_b","rust_client","mixed_case","0123456789","a"))
    val (response,data)=Login.decodeFromBuffer(ByteBuffer.wrap(packet))
    return linkedMapOf("response" to response.ordinal,"info" to info(data),"onlineLookups" to Repository.lookups.toList())
}
private fun frameCase(id: String, packet: ByteArray, online: Boolean=false) {
    emit("kind" to "decode","id" to id,"packet" to packet,"online" to online,"expected" to decoded(packet,online))
}
private fun length(packet: ByteArray): ByteArray {
    val out=packet.clone();if(out.size >= 3) {out[1]=((out.size-3) ushr 8).toByte();out[2]=(out.size-3).toByte()};return out
}
private fun secret(seed: IntArray, name: Long, password: ByteArray, magic: Int=10): ByteArray {
    val out=ByteBuffer.allocate(2048);out.put(magic.toByte());for(word in seed)out.putInt(word)
    out.putLong(name);out.put(password);out.put(0);out.put(byteArrayOf(0,0,0))
    return out.array().copyOf(out.position())
}
fun main() {
    val rng=Random(530)
    val names=listOf("","Alice","agent_42","BOB","zzzzzzzzzzzz","__a__b__","Rust Client","Mixed_Case","0123456789","abcdefghijklmnop")
    for(name in names) {
        val raw=name.toByteArray(Charsets.ISO_8859_1);val value=WriterSupport.name(raw)
        emit("kind" to "name","raw" to raw,"encoded" to value,"decoded" to StringUtils.longToString(value))
    }
    for(i in 0 until 256) {
        val raw=byteArrayOf(65,i.toByte(),90);val value=WriterSupport.name(raw)
        emit("kind" to "name","raw" to raw,"encoded" to value,"decoded" to StringUtils.longToString(value))
    }
    repeat(200) {
        val raw=ByteArray(rng.nextInt(33));rng.nextBytes(raw);val value=WriterSupport.name(raw)
        emit("kind" to "name","raw" to raw,"encoded" to value,"decoded" to StringUtils.longToString(value))
    }
    val longs=mutableListOf(Long.MIN_VALUE,-1,0,1,36,37,38,6582952005840035280L,6582952005840035281L,Long.MAX_VALUE)
    repeat(200) {longs.add(rng.nextLong())}
    for(value in longs)emit("kind" to "long_name","value" to value,"decoded" to StringUtils.longToString(value))
    for(i in 1..255) {
        val raw=byteArrayOf(i.toByte());val value=ByteBufferUtils.getString(ByteBuffer.wrap(raw+byteArrayOf(0)))
        emit("kind" to "server_string","raw" to raw,"value" to value)
    }
    val moduli=listOf("2","257","3233",GlobalConfig.RSA_MODULUS.toString())
    for(size in listOf(1,2,8,16,31,63,120))for(negative in listOf(false,true))for(mod in moduli)for(exp in listOf("0","1","17","65537")) {
        val raw=ByteArray(size);rng.nextBytes(raw);raw[0]=if(negative)0x80.toByte() else 0x7f
        val encoded=WriterSupport.rsa(raw,exp,mod)
        emit("kind" to "rsa","raw" to raw,"exp" to exp,"mod" to mod,"framed" to encoded)
    }
    val seeds=intArrayOf(Int.MIN_VALUE,Int.MAX_VALUE,0x12345678,-1)
    var base=byteArrayOf();var offset=0
    for(i in 0 until 48) {
        val name=names[i%names.size].toByteArray(Charsets.ISO_8859_1)
        val password=if(i%3==0) byteArrayOf(112,97,115,115,0x80.toByte(),0xff.toByte()) else "test_password".toByteArray()
        val fields=intArrayOf(i%5, i%2, intArrayOf(0,1,2,127,128,255,-1,256)[i%8],
            intArrayOf(765,32767,32768,65535,-1,1920)[i%6],intArrayOf(503,720,-1,65535)[i%4],
            intArrayOf(0,2,255,-128)[i%4],rng.nextInt(),rng.nextInt(),rng.nextInt(),if(i%2==0)20 else 40)
        val uid=ByteArray(24) {(it*17+i).toByte()};val settings=if(i%3==0) byteArrayOf() else byteArrayOf(99,102,103,0x80.toByte())
        val crcs=IntArray(29) {rng.nextInt()};val extra=i%3!=1;val fake=i%7!=0
        val seed=if(i%2==0)seeds else IntArray(4){rng.nextInt()}
        val packet=WriterSupport.write(name,password,seed,fields,uid,settings,crcs,extra,fake)
        val online=i%5==0
        emit("kind" to "client_frame","id" to i,"username" to name,"password" to password,
            "seed" to seed,"fields" to fields,"uid" to uid,"settings" to settings,"crcs" to crcs,
            "extra" to extra,"fake28" to fake,"packet" to packet,"online" to online,"expected" to decoded(packet,online))
        if(i==2){base=packet;offset=3+164+settings.size}
    }
    for(size in 0 until base.size) {
        frameCase("truncated_$size",base.copyOf(size));frameCase("truncated_relength_$size",length(base.copyOf(size)))
    }
    for(opcode in listOf(0,14,16,18,128,255))for(revision in listOf(0,529,530,531,-1)) {
        val packet=base.clone();packet[0]=opcode.toByte();ByteBuffer.wrap(packet).putInt(3,revision)
        frameCase("opcode_${opcode}_rev_$revision",packet)
    }
    for(name in listOf(-1L,0L,37L,Long.MAX_VALUE,Long.MIN_VALUE))for(magic in listOf(0,10,127,128,255)) {
        val encrypted=WriterSupport.rsa(secret(seeds,name,"password".toByteArray(),magic),"65537",GlobalConfig.RSA_MODULUS.toString())
        frameCase("rsa_magic_${magic}_name_$name",length(base.copyOf(offset)+encrypted),true)
    }
    for(raw in listOf(byteArrayOf(),byteArrayOf(0),byteArrayOf(1),byteArrayOf(-1),byteArrayOf(10),byteArrayOf(10,0,0))) {
        val encrypted=if(raw.isEmpty())byteArrayOf(0) else WriterSupport.rsa(raw,"65537",GlobalConfig.RSA_MODULUS.toString())
        frameCase("short_rsa_${hex(raw)}",length(base.copyOf(offset)+encrypted))
    }
    for(size in listOf(0,1,2,127,128,255)) {
        val packet=base.clone();packet[offset]=size.toByte();frameCase("rsa_length_$size",packet)
    }
    val unterminated=secret(seeds,1,byteArrayOf(65));val noTerminator=unterminated.copyOf(unterminated.size-4)
    frameCase("password_without_terminator",length(base.copyOf(offset)+WriterSupport.rsa(noTerminator,"65537",GlobalConfig.RSA_MODULUS.toString())))
    for(i in 0 until 23)for(key in listOf(Long.MIN_VALUE,-1,0,255,256,Long.MAX_VALUE)) {
        emit("kind" to "reply","response" to i,"rights" to 2,"index" to 2047,"key" to key,"bytes" to LoginReplies.encode(i,2,2047,key))
    }
    for(rights in listOf(-1,0,1,2,255,256))for(index in listOf(-1,0,1,2047,65535,65536)) {
        emit("kind" to "reply","response" to 2,"rights" to rights,"index" to index,"key" to 530,"bytes" to LoginReplies.encode(2,rights,index,530))
    }
    // Directly exercise original decryptRSABuffer underflow's distinct sentinel path.
    for(raw in listOf(byteArrayOf(),byteArrayOf(1),byteArrayOf(2,0),byteArrayOf(1,-1))) {
        val buffer=Login.decryptRSABuffer(ByteBuffer.wrap(raw),ServerConstants.EXPONENT,ServerConstants.MODULUS)
        val bytes=ByteArray(buffer.remaining());buffer.get(bytes)
        emit("kind" to "rsa_decrypt","framed" to raw,"result" to bytes)
    }
}

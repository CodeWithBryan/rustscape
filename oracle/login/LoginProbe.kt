import core.net.packet.`in`.Login
import core.game.world.repository.Repository
import core.ServerConstants
import rt4.WriterSupport
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.io.DataInputStream
import java.nio.ByteBuffer
import java.security.MessageDigest

/** Real sockets around original codec methods; no account backend or game simulation. */
object LoginProbe {
    private fun hex(bytes: ByteArray)=bytes.joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun report(direction: String,count: Int,hash: MessageDigest) {
        println("{\"direction\":\"$direction\",\"requests\":$count,\"sha256\":\"${hex(hash.digest())}\"}")
    }
    @JvmStatic fun main(args: Array<String>) {
        if(args[0]=="client") client(args[1],args[2].toInt()) else server()
    }
    private fun client(host: String,port: Int) {
        val hash=MessageDigest.getInstance("SHA-256")
        for(i in 0 until 64) Socket(host,port).use { socket ->
            socket.soTimeout=5000;socket.tcpNoDelay=true
            val output=socket.getOutputStream();val input=DataInputStream(socket.getInputStream())
            output.write(byteArrayOf(14,0));output.flush();check(input.readUnsignedByte()==0)
            val nonce=input.readLong();val seed=intArrayOf(i,Int.MAX_VALUE-i,(nonce shr 32).toInt(),nonce.toInt())
            val fields=intArrayOf(0,1,2,1280,720,2,0,1234,17,if(i%2==0)20 else 40)
            val packet=WriterSupport.write("Rust_Client".toByteArray(),"test_password".toByteArray(),seed,fields,
                ByteArray(24) {it.toByte()},byteArrayOf(),IntArray(29){it*257},i%3!=0,i%11!=0)
            if(i%9==0) ByteBuffer.wrap(packet).putInt(3,529)
            if(i%13==0) packet[0]=0
            Repository.online.clear()
            val (decoded,_)=Login.decodeFromBuffer(ByteBuffer.wrap(packet))
            val expected=if(decoded.ordinal==2)8 else decoded.ordinal
            for(chunk in packet.asList().chunked(13)) {output.write(chunk.toByteArray());output.flush()}
            val response=input.readUnsignedByte();check(response==expected) {"Rust replied $response, expected $expected for original client case $i"}
            check(input.read()==-1);hash.update(packet);hash.update(response.toByte())
        }
        report("jvm_to_native",64,hash)
    }
    private fun server() {
        val hash=MessageDigest.getInstance("SHA-256")
        ServerSocket(0,50,InetAddress.getByName("127.0.0.1")).use { listener ->
            listener.soTimeout=10000
            println("{\"port\":${listener.localPort}}");System.out.flush()
            val codes=intArrayOf(2,6,21,23,29,0,2,8)
            for(i in codes.indices) listener.accept().use { socket ->
                socket.soTimeout=5000;socket.tcpNoDelay=true
                val input=DataInputStream(socket.getInputStream());val output=socket.getOutputStream()
                check(input.readUnsignedByte()==14);check(input.readUnsignedByte()==((WriterSupport.name("Rust_Client".toByteArray()) shr 16) and 31).toInt())
                val nonce=0x12345678abcdef00L+i
                val ack=ByteBuffer.allocate(9).put(0).putLong(nonce).array()
                for(byte in ack) {output.write(byte.toInt() and 255);output.flush()}
                val op=input.readUnsignedByte();val size=input.readUnsignedShort();val packet=ByteArray(size+3)
                packet[0]=op.toByte();packet[1]=(size ushr 8).toByte();packet[2]=size.toByte();input.readFully(packet,3,size)
                Repository.online.clear();val (response,info)=Login.decodeFromBuffer(ByteBuffer.wrap(packet))
                check(response.ordinal==2 && info!=null);check(info.username=="rust_client" && info.password=="test_password")
                check(info.opcode==if(i%2==0)16 else 18);check(info.screenWidth==765 && info.screenHeight==503)
                val rsa=ByteBuffer.wrap(packet);rsa.position(167)
                val secret=Login.decryptRSABuffer(rsa,ServerConstants.EXPONENT,ServerConstants.MODULUS)
                check(secret.get().toInt()==10);check(secret.int==17 && secret.int==27)
                check(secret.int==(nonce shr 32).toInt() && secret.int==nonce.toInt())
                val code=codes[i]
                // Codes 23/29 are client retry/detail paths absent from the server enum.
                val reply=when(code) {23 -> byteArrayOf(23);29 -> byteArrayOf(29,17);else -> LoginReplies.encode(code,2,2047,nonce)}
                output.write(reply);if(code==2)output.write(byteArrayOf(1,0,2,0xaa.toByte(),0xbb.toByte()));output.flush()
                hash.update(packet);hash.update(reply)
            }
        }
        report("native_to_jvm",8,hash)
    }
}

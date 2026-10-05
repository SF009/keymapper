package io.sf009.waydroidkeymapper.input

import java.io.DataInputStream
import java.io.InputStream

object Protocol {
    const val MAGIC = 0x57444B4D
    const val VERSION = 1
    const val HELLO = 1
    const val KEY = 2
    const val MOUSE_MOVE = 3
    const val MOUSE_BUTTON = 4
    const val PING = 5

    data class Frame(val type: Int, val payload: ByteArray)

    fun read(input: InputStream): Frame? {
        return try {
            val d = DataInputStream(input)
            if (d.readInt() != MAGIC) return null
            if (d.readUnsignedByte() != VERSION) return null
            val type = d.readUnsignedByte()
            val len = d.readUnsignedShort()
            if (len > 4096) return null
            val payload = ByteArray(len)
            d.readFully(payload)
            Frame(type, payload)
        } catch (_: Exception) {
            null
        }
    }
}

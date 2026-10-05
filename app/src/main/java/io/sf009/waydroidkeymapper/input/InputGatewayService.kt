package io.sf009.waydroidkeymapper.input

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import io.sf009.waydroidkeymapper.model.ProfileStore
import java.io.BufferedInputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

class InputGatewayService : Service() {
    companion object {
        const val PORT = 27183
        private const val CHANNEL = "waydroid-keymapper"
    }

    private val alive = AtomicBoolean(false)
    private val pool = Executors.newCachedThreadPool()
    private var server: ServerSocket? = null
    private lateinit var injector: TouchInjector

    override fun onCreate() {
        super.onCreate()
        createChannel()
        startForeground(1001, notification())
        injector = TouchInjector { ProfileStore(this).active() }
        alive.set(true)
        server = ServerSocket(PORT, 8, InetAddress.getByName("127.0.0.1"))
        pool.execute {
            while (alive.get()) {
                val s = runCatching { server?.accept() }.getOrNull() ?: break
                pool.execute { serve(s) }
            }
        }
    }

    private fun serve(socket: Socket) {
        socket.use {
            it.tcpNoDelay = true
            val input = BufferedInputStream(it.getInputStream(), 4096)
            while (alive.get()) {
                val f = Protocol.read(input) ?: break
                when (f.type) {
                    Protocol.KEY -> if (f.payload.size >= 3) {
                        val code = ((f.payload[0].toInt() and 255) shl 8) or (f.payload[1].toInt() and 255)
                        injector.key(code, f.payload[2].toInt() != 0)
                    }
                    Protocol.MOUSE_MOVE -> if (f.payload.size >= 4) {
                        val dx = ((f.payload[0].toInt() shl 8) or (f.payload[1].toInt() and 255)).toShort().toInt()
                        val dy = ((f.payload[2].toInt() shl 8) or (f.payload[3].toInt() and 255)).toShort().toInt()
                        injector.mouseMove(dx, dy)
                    }
                    Protocol.MOUSE_BUTTON -> if (f.payload.size >= 3) {
                        val code = ((f.payload[0].toInt() and 255) shl 8) or (f.payload[1].toInt() and 255)
                        injector.mouseButton(code, f.payload[2].toInt() != 0)
                    }
                    else -> Unit
                }
            }
        }
    }

    override fun onDestroy() {
        alive.set(false)
        runCatching { server?.close() }
        injector.stopAll()
        pool.shutdownNow()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createChannel() {
        if (Build.VERSION.SDK_INT >= 26) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(CHANNEL, "Waydroid Keymapper", NotificationManager.IMPORTANCE_LOW)
            )
        }
    }

    private fun notification(): Notification =
        if (Build.VERSION.SDK_INT >= 26) {
            Notification.Builder(this, CHANNEL)
                .setContentTitle("Waydroid Keymapper")
                .setContentText("Input gateway :27183")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .build()
        } else {
            Notification.Builder(this)
                .setContentTitle("Waydroid Keymapper")
                .setContentText("Input gateway :27183")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .build()
        }
}

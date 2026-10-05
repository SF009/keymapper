package io.sf009.waydroidkeymapper.input

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import io.sf009.waydroidkeymapper.model.Profile
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

        @Volatile
        private var cachedProfile: Profile? = null

        fun setActiveProfile(profile: Profile) {
            cachedProfile = profile
        }
    }

    private val alive = AtomicBoolean(false)
    private val pool = Executors.newCachedThreadPool()
    private var server: ServerSocket? = null
    private lateinit var injector: TouchInjector

    override fun onCreate() {
        super.onCreate()
        createChannel()

        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(
                1001,
                notification(),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
            )
        } else {
            startForeground(1001, notification())
        }

        val profile = ProfileStore(this).active()
        cachedProfile = profile
        injector = TouchInjector { cachedProfile ?: profile }

        alive.set(true)
        server = ServerSocket(PORT, 8, InetAddress.getByName("127.0.0.1"))

        pool.execute {
            while (alive.get()) {
                val socket = runCatching { server?.accept() }.getOrNull() ?: break
                socket?.let { pool.execute { serve(it) } }
            }
        }
    }

    private fun serve(socket: Socket) {
        socket.use { s ->
            s.tcpNoDelay = true
            s.keepAlive = true
            val input = BufferedInputStream(s.getInputStream(), 4096)

            while (alive.get()) {
                val frame = Protocol.read(input) ?: break
                when (frame.type) {
                    Protocol.HELLO,
                    Protocol.PING -> Unit

                    Protocol.KEY -> if (frame.payload.size >= 3) {
                        val code = ((frame.payload[0].toInt() and 255) shl 8) or
                            (frame.payload[1].toInt() and 255)
                        injector.key(code, frame.payload[2].toInt() and 255)
                    }

                    Protocol.MOUSE_MOVE -> if (frame.payload.size >= 4) {
                        val dx = java.nio.ByteBuffer.wrap(frame.payload, 0, 2).short.toInt()
                        val dy = java.nio.ByteBuffer.wrap(frame.payload, 2, 2).short.toInt()
                        injector.mouseMove(dx, dy)
                    }

                    Protocol.MOUSE_BUTTON -> if (frame.payload.size >= 3) {
                        val code = ((frame.payload[0].toInt() and 255) shl 8) or
                            (frame.payload[1].toInt() and 255)
                        injector.mouseButton(code, frame.payload[2].toInt() and 255)
                    }
                }
            }
        }
    }

    override fun onDestroy() {
        alive.set(false)
        runCatching { server?.close() }
        if (::injector.isInitialized) injector.stopAll()
        pool.shutdownNow()
        cachedProfile = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createChannel() {
        if (Build.VERSION.SDK_INT >= 26) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(
                    CHANNEL,
                    "Waydroid Keymapper",
                    NotificationManager.IMPORTANCE_LOW
                )
            )
        }
    }

    private fun notification(): Notification =
        if (Build.VERSION.SDK_INT >= 26) {
            Notification.Builder(this, CHANNEL)
                .setContentTitle("Waydroid Keymapper")
                .setContentText("Input gateway :$PORT")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .build()
        } else {
            Notification.Builder(this)
                .setContentTitle("Waydroid Keymapper")
                .setContentText("Input gateway :$PORT")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .build()
        }
}

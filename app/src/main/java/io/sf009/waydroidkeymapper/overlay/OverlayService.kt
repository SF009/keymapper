package io.sf009.waydroidkeymapper.overlay

import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import io.sf009.waydroidkeymapper.model.Profile
import io.sf009.waydroidkeymapper.model.ProfileStore

class OverlayService : Service() {
    private var view: OverlayView? = null
    private var wm: WindowManager? = null
    private val main = Handler(Looper.getMainLooper())
    private val refresh = object : Runnable {
        override fun run() {
            view?.reload()
            if (view != null) main.postDelayed(this, 250L)
        }
    }

    override fun onCreate() {
        super.onCreate()
        if (!Settings.canDrawOverlays(this)) return

        wm = getSystemService(WINDOW_SERVICE) as WindowManager
        val flags = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
            WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
            WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED

        val lp = WindowManager.LayoutParams(
            -1,
            -1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            flags,
            android.graphics.PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            title = "Waydroid Keymapper Overlay"
        }

        view = OverlayView(this)
        wm?.addView(view, lp)
        main.post(refresh)
    }

    override fun onDestroy() {
        main.removeCallbacks(refresh)
        runCatching { view?.let { wm?.removeView(it) } }
        view = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private class OverlayView(ctx: Context) : View(ctx) {
        private val store = ProfileStore(ctx)
        private var profile: Profile = store.active()
        private val p = Paint(Paint.ANTI_ALIAS_FLAG)

        fun reload() {
            profile = store.active()
            invalidate()
        }

        override fun onDraw(c: Canvas) {
            super.onDraw(c)

            val w = width.toFloat()
            val h = height.toFloat()
            if (w <= 0f || h <= 0f) return

            fun pxX(v: Float) = v.coerceIn(0f, 1f) * w
            fun pxY(v: Float) = v.coerceIn(0f, 1f) * h

            if (profile.joystickEnabled) {
                p.style = Paint.Style.STROKE
                p.strokeWidth = 3f
                p.color = Color.argb(150, 80, 240, 140)
                c.drawCircle(
                    pxX(profile.joystickX),
                    pxY(profile.joystickY),
                    minOf(w, h) * profile.joystickRadius,
                    p
                )
                p.style = Paint.Style.FILL
                p.color = Color.argb(90, 80, 240, 140)
                c.drawCircle(pxX(profile.joystickX), pxY(profile.joystickY), 8f, p)
            }

            if (profile.aimEnabled) {
                p.style = Paint.Style.STROKE
                p.strokeWidth = 3f
                p.color = Color.argb(185, 255, 65, 85)
                val ax = pxX(profile.aimX)
                val ay = pxY(profile.aimY)
                c.drawCircle(ax, ay, 26f, p)
                c.drawLine(ax - 19f, ay, ax + 19f, ay, p)
                c.drawLine(ax, ay - 19f, ax, ay + 19f, p)
            }

            p.typeface = Typeface.DEFAULT_BOLD
            p.textSize = 17f
            p.style = Paint.Style.FILL

            profile.bindings.forEach {
                val bx = pxX(it.x)
                val by = pxY(it.y)
                p.color = Color.argb(80, 255, 255, 255)
                c.drawCircle(bx, by, 20f, p)
                p.color = Color.WHITE
                c.drawText(it.label, bx + 26f, by + 6f, p)
            }
        }
    }
}

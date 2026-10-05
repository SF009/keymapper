package io.sf009.waydroidkeymapper.overlay

import android.app.Service
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.os.IBinder
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import io.sf009.waydroidkeymapper.model.ProfileStore

class OverlayService : Service() {
    private var view: View? = null
    private var wm: WindowManager? = null

    override fun onCreate() {
        super.onCreate()
        if (!Settings.canDrawOverlays(this)) return
        wm = getSystemService(WINDOW_SERVICE) as WindowManager
        val flags = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
            WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
        val lp = WindowManager.LayoutParams(
            -1, -1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            flags,
            android.graphics.PixelFormat.TRANSLUCENT
        )
        lp.gravity = Gravity.TOP or Gravity.START
        view = OverlayView(this)
        wm?.addView(view, lp)
    }

    override fun onDestroy() {
        runCatching { view?.let { wm?.removeView(it) } }
        view = null
        super.onDestroy()
    }

    override fun onBind(intent: android.content.Intent?): IBinder? = null

    private class OverlayView(ctx: Context) : View(ctx) {
        private val p = Paint(Paint.ANTI_ALIAS_FLAG)
        private val profile = ProfileStore(ctx).active()

        override fun onDraw(c: Canvas) {
            val w = width.toFloat()
            val h = height.toFloat()
            fun pxX(v: Float) = v * w
            fun pxY(v: Float) = v * h

            p.style = Paint.Style.STROKE
            p.strokeWidth = 2f
            if (profile.joystickEnabled) {
                p.color = Color.argb(120, 80, 240, 140)
                c.drawCircle(pxX(profile.joystickX), pxY(profile.joystickY), minOf(w, h) * profile.joystickRadius, p)
            }
            if (profile.aimEnabled) {
                p.color = Color.argb(140, 255, 70, 80)
                c.drawCircle(pxX(profile.aimX), pxY(profile.aimY), 24f, p)
                c.drawLine(pxX(profile.aimX) - 16, pxY(profile.aimY), pxX(profile.aimX) + 16, pxY(profile.aimY), p)
                c.drawLine(pxX(profile.aimX), pxY(profile.aimY) - 16, pxX(profile.aimX), pxY(profile.aimY) + 16, p)
            }
            p.typeface = Typeface.DEFAULT_BOLD
            p.textSize = 18f
            p.style = Paint.Style.FILL
            profile.bindings.forEach {
                p.color = Color.argb(85, 255, 255, 255)
                c.drawCircle(pxX(it.x), pxY(it.y), 18f, p)
                p.color = Color.WHITE
                c.drawText(it.label, pxX(it.x) + 22f, pxY(it.y) + 6f, p)
            }
        }
    }
}

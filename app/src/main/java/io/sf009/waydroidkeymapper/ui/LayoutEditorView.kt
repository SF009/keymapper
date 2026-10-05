package io.sf009.waydroidkeymapper.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.view.MotionEvent
import android.view.View
import io.sf009.waydroidkeymapper.model.Binding
import io.sf009.waydroidkeymapper.model.Profile
import kotlin.math.hypot

class LayoutEditorView(context: Context) : View(context) {
    var profile: Profile? = null
        set(value) { field = value; invalidate() }

    private var selected: Binding? = null
    private var selectedSpecial: Int = 0 // 1=joystick, 2=aim
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)

    override fun onDraw(c: Canvas) {
        val p = profile ?: return
        paint.style = Paint.Style.FILL
        paint.color = Color.rgb(16, 18, 23)
        c.drawRect(0f, 0f, width.toFloat(), height.toFloat(), paint)

        paint.style = Paint.Style.STROKE
        paint.strokeWidth = 1f
        paint.color = Color.rgb(42, 45, 55)
        for (i in 1 until 10) {
            val x = width * i / 10f
            val y = height * i / 10f
            c.drawLine(x, 0f, x, height.toFloat(), paint)
            c.drawLine(0f, y, width.toFloat(), y, paint)
        }

        if (p.joystickEnabled) {
            paint.color = Color.argb(160, 80, 230, 130)
            c.drawCircle(p.joystickX * width, p.joystickY * height, minOf(width, height) * p.joystickRadius, paint)
        }
        if (p.aimEnabled) {
            paint.color = Color.argb(220, 255, 70, 80)
            c.drawCircle(p.aimX * width, p.aimY * height, 24f, paint)
        }

        paint.style = Paint.Style.FILL
        paint.textSize = 18f
        p.bindings.forEach {
            paint.color = if (it == selected) Color.rgb(255, 80, 90) else Color.rgb(70, 110, 160)
            c.drawCircle(it.x * width, it.y * height, 22f, paint)
            paint.color = Color.WHITE
            c.drawText(it.label, it.x * width + 28f, it.y * height + 6f, paint)
        }
    }

    override fun onTouchEvent(e: MotionEvent): Boolean {
        val p = profile ?: return false
        val nx = (e.x / width.toFloat()).coerceIn(0f, 1f)
        val ny = (e.y / height.toFloat()).coerceIn(0f, 1f)
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> { selected = nearest(p, nx, ny); return true }
            MotionEvent.ACTION_MOVE -> {
                selected?.let { it.x = nx; it.y = ny; invalidate() }
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> { selected = null; return true }
        }
        return true
    }

    private fun nearest(p: Profile, x: Float, y: Float): Binding? {
        var best: Binding? = null
        var d = 0.05
        p.bindings.forEach {
            val n = hypot((it.x - x).toDouble(), (it.y - y).toDouble())
            if (n < d) { d = n; best = it }
        }
        return best
    }
}

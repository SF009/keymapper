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
import kotlin.math.min

class LayoutEditorView(context: Context) : View(context) {
    var profile: Profile? = null
        set(value) {
            field = value
            selected = null
            selectedKind = 0
            invalidate()
        }

    private var selected: Binding? = null
    private var selectedKind: Int = 0 // 0=binding, 1=joystick, 2=aim
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)

    override fun onDraw(c: Canvas) {
        val p = profile ?: return
        val w = width.toFloat()
        val h = height.toFloat()
        if (w <= 0f || h <= 0f) return

        paint.style = Paint.Style.FILL
        paint.color = Color.rgb(14, 16, 21)
        c.drawRect(0f, 0f, w, h, paint)

        paint.style = Paint.Style.STROKE
        paint.strokeWidth = 1f
        paint.color = Color.rgb(40, 44, 52)
        for (i in 1 until 10) {
            c.drawLine(w * i / 10f, 0f, w * i / 10f, h, paint)
            c.drawLine(0f, h * i / 10f, w, h * i / 10f, paint)
        }

        if (p.joystickEnabled) {
            paint.color = Color.argb(170, 80, 230, 130)
            c.drawCircle(
                p.joystickX * w,
                p.joystickY * h,
                min(w, h) * p.joystickRadius,
                paint
            )
            paint.color = Color.argb(80, 80, 230, 130)
            c.drawCircle(p.joystickX * w, p.joystickY * h, 7f, paint)
        }

        if (p.aimEnabled) {
            paint.color = Color.argb(220, 255, 70, 80)
            c.drawCircle(p.aimX * w, p.aimY * h, 25f, paint)
            c.drawLine(p.aimX * w - 18f, p.aimY * h, p.aimX * w + 18f, p.aimY * h, paint)
            c.drawLine(p.aimX * w, p.aimY * h - 18f, p.aimX * w, p.aimY * h + 18f, paint)
        }

        p.bindings.forEach {
            paint.style = Paint.Style.FILL
            paint.color = if (it == selected) Color.rgb(255, 80, 95) else Color.rgb(64, 106, 160)
            c.drawCircle(it.x * w, it.y * h, 22f, paint)
            paint.color = Color.WHITE
            paint.textSize = 17f
            c.drawText(it.label, it.x * w + 28f, it.y * h + 6f, paint)
        }
    }

    override fun onTouchEvent(e: MotionEvent): Boolean {
        val p = profile ?: return false
        val nx = (e.x / width.toFloat()).coerceIn(0f, 1f)
        val ny = (e.y / height.toFloat()).coerceIn(0f, 1f)

        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                val hit = nearest(p, nx, ny)
                selected = hit.first
                selectedKind = hit.second
                invalidate()
                return true
            }

            MotionEvent.ACTION_MOVE -> {
                when (selectedKind) {
                    0 -> selected?.let {
                        it.x = nx
                        it.y = ny
                    }
                    1 -> {
                        p.joystickX = nx
                        p.joystickY = ny
                    }
                    2 -> {
                        p.aimX = nx
                        p.aimY = ny
                    }
                }
                invalidate()
                return true
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                selected = null
                selectedKind = 0
                invalidate()
                return true
            }
        }
        return true
    }

    private fun nearest(p: Profile, x: Float, y: Float): Pair<Binding?, Int> {
        var bestBinding: Binding? = null
        var best = 0.045

        p.bindings.forEach {
            val d = hypot((it.x - x).toDouble(), (it.y - y).toDouble())
            if (d < best) {
                best = d
                bestBinding = it
            }
        }

        if (p.joystickEnabled) {
            val d = hypot((p.joystickX - x).toDouble(), (p.joystickY - y).toDouble())
            if (d < best + p.joystickRadius) return null to 1
        }

        if (p.aimEnabled) {
            val d = hypot((p.aimX - x).toDouble(), (p.aimY - y).toDouble())
            if (d < maxOf(best, 0.04)) return null to 2
        }

        return bestBinding to 0
    }
}

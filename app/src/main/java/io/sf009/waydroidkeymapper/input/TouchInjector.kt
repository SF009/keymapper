package io.sf009.waydroidkeymapper.input

import android.os.Handler
import android.os.Looper
import io.sf009.waydroidkeymapper.model.Binding
import io.sf009.waydroidkeymapper.model.Profile
import io.sf009.waydroidkeymapper.model.Trigger
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max

class TouchInjector(private val profileProvider: () -> Profile) {
    private val main = Handler(Looper.getMainLooper())
    private val holds = linkedMapOf<Int, Binding>()
    private var aimActive = false
    private var aimX = 0f
    private var aimY = 0f
    private var dxQueued = 0
    private var dyQueued = 0
    private var aimPosted = false
    private val closed = AtomicBoolean(false)

    private fun service() = WaydroidAccessibilityService.instance()

    private fun x(v: Float, p: Profile) =
        (v.coerceIn(0f, 1f) * p.displayWidth)
            .coerceIn(0f, p.displayWidth.toFloat())

    private fun y(v: Float, p: Profile) =
        (v.coerceIn(0f, 1f) * p.displayHeight)
            .coerceIn(0f, p.displayHeight.toFloat())

    @Synchronized
    fun key(code: Int, state: Int) {
        if (closed.get()) return
        val p = profileProvider()
        val b = p.bindings.firstOrNull {
            it.source == "key" && it.code == code
        } ?: return

        if (b.trigger == Trigger.TAP) {
            if (state == 1) service()?.tap(x(b.x, p), y(b.y, p))
            return
        }

        when (state) {
            1 -> holds[code] = b
            0 -> holds.remove(code)
        }
        if (state != 2) refreshHolds(p)
    }

    @Synchronized
    fun mouseButton(code: Int, state: Int) {
        if (closed.get()) return
        val p = profileProvider()

        if (p.aimEnabled && code == p.aimButton) {
            aimActive = state != 0
            if (aimActive) {
                aimX = x(p.aimX, p)
                aimY = y(p.aimY, p)
            } else {
                dxQueued = 0
                dyQueued = 0
            }
            return
        }

        val b = p.bindings.firstOrNull {
            it.source == "mouse" && it.code == code
        } ?: return

        if (b.trigger == Trigger.TAP) {
            if (state == 1) service()?.tap(x(b.x, p), y(b.y, p))
            return
        }

        val id = 10_000 + code
        when (state) {
            1 -> holds[id] = b
            0 -> holds.remove(id)
        }
        if (state != 2) refreshHolds(p)
    }

    fun mouseMove(dx: Int, dy: Int) {
        if (closed.get()) return
        synchronized(this) {
            if (!aimActive) return
            dxQueued = (dxQueued + dx).coerceIn(-32768, 32767)
            dyQueued = (dyQueued + dy).coerceIn(-32768, 32767)
            if (!aimPosted) {
                aimPosted = true
                main.postDelayed(::flushAim, 2L)
            }
        }
    }

    private fun flushAim() {
        val p = profileProvider()
        val pair = synchronized(this) {
            val result = dxQueued to dyQueued
            dxQueued = 0
            dyQueued = 0
            aimPosted = false
            result
        }

        if ((pair.first == 0 && pair.second == 0) || !aimActive || closed.get()) return

        val scale = max(0.05f, p.aimSensitivity)
        val tx = (aimX + pair.first * scale)
            .coerceIn(0f, p.displayWidth.toFloat())
        val ty = (
            aimY + (if (p.invertY) -pair.second else pair.second) * scale
        ).coerceIn(0f, p.displayHeight.toFloat())

        service()?.swipe(aimX, aimY, tx, ty, 2L)
        aimX = tx
        aimY = ty
    }

    @Synchronized
    private fun refreshHolds(p: Profile) {
        val s = service() ?: return
        if (holds.isEmpty()) {
            s.cancelAll()
            return
        }

        val builder = android.accessibilityservice.GestureDescription.Builder()
        holds.values.take(10).forEach {
            val path = android.graphics.Path().apply {
                moveTo(x(it.x, p), y(it.y, p))
            }
            builder.addStroke(
                android.accessibilityservice.GestureDescription.StrokeDescription(
                    path,
                    0,
                    60_000
                )
            )
        }
        s.dispatchGesture(builder.build(), null, null)
    }

    fun stopAll() {
        synchronized(this) {
            if (closed.getAndSet(true)) return
            holds.clear()
            aimActive = false
            dxQueued = 0
            dyQueued = 0
            aimPosted = false
        }
        main.removeCallbacks(::flushAim)
        service()?.cancelAll()
    }
}

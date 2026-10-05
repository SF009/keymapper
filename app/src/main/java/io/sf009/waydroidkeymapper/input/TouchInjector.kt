package io.sf009.waydroidkeymapper.input

import android.os.Handler
import android.os.Looper
import io.sf009.waydroidkeymapper.model.Binding
import io.sf009.waydroidkeymapper.model.Profile
import io.sf009.waydroidkeymapper.model.Trigger
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

    private fun service() = WaydroidAccessibilityService.instance()

    private fun x(v: Float, p: Profile) = (v.coerceIn(0f, 1f) * p.displayWidth).coerceIn(0f, p.displayWidth.toFloat())
    private fun y(v: Float, p: Profile) = (v.coerceIn(0f, 1f) * p.displayHeight).coerceIn(0f, p.displayHeight.toFloat())

    @Synchronized fun key(code: Int, down: Boolean) {
        val p = profileProvider()
        val b = p.bindings.firstOrNull { it.source == "key" && it.code == code } ?: return
        if (b.trigger == Trigger.TAP) {
            if (down) service()?.tap(x(b.x, p), y(b.y, p))
            return
        }
        if (down) holds[code] = b else holds.remove(code)
        refreshHolds(p)
    }

    @Synchronized fun mouseButton(code: Int, down: Boolean) {
        val p = profileProvider()
        if (p.aimEnabled && code == p.aimButton) {
            aimActive = down
            if (down) {
                aimX = x(p.aimX, p)
                aimY = y(p.aimY, p)
            } else {
                dxQueued = 0
                dyQueued = 0
                service()?.cancelAll()
            }
            return
        }

        val b = p.bindings.firstOrNull { it.source == "mouse" && it.code == code } ?: return
        if (b.trigger == Trigger.TAP) {
            if (down) service()?.tap(x(b.x, p), y(b.y, p))
            return
        }
        val id = 10_000 + code
        if (down) holds[id] = b else holds.remove(id)
        refreshHolds(p)
    }

    fun mouseMove(dx: Int, dy: Int) {
        synchronized(this) {
            if (!aimActive) return
            dxQueued += dx
            dyQueued += dy
            if (!aimPosted) {
                aimPosted = true
                main.postDelayed(::flushAim, 4)
            }
        }
    }

    private fun flushAim() {
        val p = profileProvider()
        val pair = synchronized(this) {
            val r = dxQueued to dyQueued
            dxQueued = 0
            dyQueued = 0
            aimPosted = false
            r
        }
        if ((pair.first == 0 && pair.second == 0) || !aimActive) return

        val scale = max(.05f, p.aimSensitivity)
        val tx = (aimX + pair.first * scale).coerceIn(0f, p.displayWidth.toFloat())
        val ty = (aimY + (if (p.invertY) -pair.second else pair.second) * scale)
            .coerceIn(0f, p.displayHeight.toFloat())

        service()?.swipe(aimX, aimY, tx, ty, 4)
        aimX = tx
        aimY = ty
    }

    @Synchronized private fun refreshHolds(p: Profile) {
        val s = service() ?: return
        if (holds.isEmpty()) {
            s.cancelAll()
            return
        }
        val b = android.accessibilityservice.GestureDescription.Builder()
        holds.values.take(10).forEach {
            val path = android.graphics.Path().apply { moveTo(x(it.x, p), y(it.y, p)) }
            b.addStroke(android.accessibilityservice.GestureDescription.StrokeDescription(path, 0, 60_000))
        }
        s.dispatchGesture(b.build(), null, null)
    }

    fun stopAll() {
        synchronized(this) {
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

package io.sf009.waydroidkeymapper.input

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.AccessibilityServiceInfo
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.view.accessibility.AccessibilityEvent
import java.util.concurrent.atomic.AtomicReference

class WaydroidAccessibilityService : AccessibilityService() {
    companion object {
        private val ref = AtomicReference<WaydroidAccessibilityService?>(null)
        fun instance() = ref.get()
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        serviceInfo = serviceInfo.apply {
            flags = flags or AccessibilityServiceInfo.FLAG_REQUEST_FILTER_KEY_EVENTS
        }
        ref.set(this)
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) = Unit

    override fun onInterrupt() {
        ref.compareAndSet(this, null)
    }

    override fun onDestroy() {
        ref.compareAndSet(this, null)
        super.onDestroy()
    }

    fun tap(x: Float, y: Float) {
        val p = Path().apply { moveTo(x, y) }
        dispatchGesture(GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(p, 0, 1))
            .build(), null, null)
    }

    fun swipe(x1: Float, y1: Float, x2: Float, y2: Float, ms: Long) {
        val p = Path().apply {
            moveTo(x1, y1)
            lineTo(x2, y2)
        }
        dispatchGesture(GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(p, 0, ms.coerceIn(1, 120)))
            .build(), null, null)
    }

    fun hold(x: Float, y: Float) = swipe(x, y, x, y, 60_000)

    fun cancelAll() {
        dispatchGesture(GestureDescription.Builder().build(), null, null)
    }
}

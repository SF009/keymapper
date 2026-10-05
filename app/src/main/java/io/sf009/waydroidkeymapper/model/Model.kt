package io.sf009.waydroidkeymapper.model

import org.json.JSONArray
import org.json.JSONObject

enum class Trigger { TAP, HOLD }

data class Binding(
    var source: String,
    var code: Int,
    var trigger: Trigger,
    var x: Float,
    var y: Float,
    var label: String,
    var slot: Int = 0,
) {
    fun toJson() = JSONObject().apply {
        put("source", source)
        put("code", code)
        put("trigger", trigger.name)
        put("x", x.toDouble())
        put("y", y.toDouble())
        put("label", label)
        put("slot", slot)
    }

    companion object {
        fun fromJson(o: JSONObject) = Binding(
            source = o.optString("source", "key"),
            code = o.optInt("code", 0),
            trigger = runCatching { Trigger.valueOf(o.optString("trigger", "TAP")) }.getOrDefault(Trigger.TAP),
            x = o.optDouble("x", .5).toFloat().coerceIn(0f, 1f),
            y = o.optDouble("y", .5).toFloat().coerceIn(0f, 1f),
            label = o.optString("label", "KEY"),
            slot = o.optInt("slot", 0).coerceIn(0, 9)
        )
    }
}

data class Profile(
    var name: String,
    var displayWidth: Int = 1920,
    var displayHeight: Int = 1080,
    var aimEnabled: Boolean = true,
    var aimButton: Int = 273,
    var aimX: Float = .5f,
    var aimY: Float = .5f,
    var aimSensitivity: Float = 2f,
    var invertY: Boolean = false,
    var joystickEnabled: Boolean = true,
    var joystickX: Float = .15f,
    var joystickY: Float = .76f,
    var joystickRadius: Float = .085f,
    val bindings: MutableList<Binding> = mutableListOf()
) {
    fun toJson() = JSONObject().apply {
        put("name", name)
        put("displayWidth", displayWidth)
        put("displayHeight", displayHeight)
        put("aimEnabled", aimEnabled)
        put("aimButton", aimButton)
        put("aimX", aimX.toDouble())
        put("aimY", aimY.toDouble())
        put("aimSensitivity", aimSensitivity.toDouble())
        put("invertY", invertY)
        put("joystickEnabled", joystickEnabled)
        put("joystickX", joystickX.toDouble())
        put("joystickY", joystickY.toDouble())
        put("joystickRadius", joystickRadius.toDouble())
        put("bindings", JSONArray().also { a -> bindings.forEach { a.put(it.toJson()) } })
    }

    companion object {
        fun fromJson(o: JSONObject): Profile {
            val p = Profile(
                name = o.optString("name", "Default"),
                displayWidth = o.optInt("displayWidth", 1920),
                displayHeight = o.optInt("displayHeight", 1080),
                aimEnabled = o.optBoolean("aimEnabled", true),
                aimButton = o.optInt("aimButton", 273),
                aimX = o.optDouble("aimX", .5).toFloat(),
                aimY = o.optDouble("aimY", .5).toFloat(),
                aimSensitivity = o.optDouble("aimSensitivity", 2.0).toFloat(),
                invertY = o.optBoolean("invertY", false),
                joystickEnabled = o.optBoolean("joystickEnabled", true),
                joystickX = o.optDouble("joystickX", .15).toFloat(),
                joystickY = o.optDouble("joystickY", .76).toFloat(),
                joystickRadius = o.optDouble("joystickRadius", .085).toFloat()
            )
            val a = o.optJSONArray("bindings") ?: JSONArray()
            for (i in 0 until a.length()) p.bindings += Binding.fromJson(a.getJSONObject(i))
            return p
        }

        fun freeFire() = Profile("Free Fire").apply {
            bindings += Binding("key", 17, Trigger.HOLD, .15f, .76f, "W", 0)
            bindings += Binding("key", 30, Trigger.HOLD, .15f, .76f, "A", 0)
            bindings += Binding("key", 31, Trigger.HOLD, .15f, .76f, "S", 0)
            bindings += Binding("key", 32, Trigger.HOLD, .15f, .76f, "D", 0)
            bindings += Binding("key", 57, Trigger.TAP, .84f, .86f, "JUMP", 2)
            bindings += Binding("key", 19, Trigger.TAP, .93f, .18f, "RELOAD", 3)
            bindings += Binding("key", 34, Trigger.TAP, .58f, .18f, "GRENADE", 4)
            bindings += Binding("key", 42, Trigger.HOLD, .28f, .76f, "SPRINT", 5)
            bindings += Binding("key", 46, Trigger.HOLD, .34f, .88f, "CROUCH", 6)
            bindings += Binding("key", 33, Trigger.HOLD, .76f, .83f, "INTERACT", 7)
            bindings += Binding("mouse", 272, Trigger.HOLD, .88f, .78f, "FIRE", 8)
        }

        fun minimal() = Profile("Minimal").apply {
            bindings += Binding("key", 17, Trigger.HOLD, .15f, .76f, "W", 0)
            bindings += Binding("key", 30, Trigger.HOLD, .15f, .76f, "A", 0)
            bindings += Binding("key", 31, Trigger.HOLD, .15f, .76f, "S", 0)
            bindings += Binding("key", 32, Trigger.HOLD, .15f, .76f, "D", 0)
            bindings += Binding("key", 57, Trigger.TAP, .84f, .86f, "JUMP", 2)
            bindings += Binding("mouse", 272, Trigger.HOLD, .88f, .78f, "FIRE", 3)
        }
    }
}

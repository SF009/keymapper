package io.sf009.waydroidkeymapper

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.widget.*
import io.sf009.waydroidkeymapper.input.InputGatewayService
import io.sf009.waydroidkeymapper.model.*
import io.sf009.waydroidkeymapper.overlay.OverlayService
import io.sf009.waydroidkeymapper.ui.LayoutEditorView

class MainActivity : Activity() {
    private lateinit var store: ProfileStore
    private lateinit var spinner: Spinner
    private lateinit var status: TextView
    private var current: Profile = Profile.freeFire()
    private var editor: LayoutEditorView? = null

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        store = ProfileStore(this)
        current = store.active()
        buildMain()
        requestNotificationPermission()
    }

    private fun buildMain() {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(20, 20, 20, 20)
            setBackgroundColor(Color.rgb(16, 17, 20))
        }
        root.addView(TextView(this).apply {
            text = "WAYDROID KEYMAPPER"
            textSize = 24f
            setTextColor(Color.WHITE)
        }, LinearLayout.LayoutParams(-1, 70))

        spinner = Spinner(this)
        root.addView(spinner, LinearLayout.LayoutParams(-1, 60))

        val buttons = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        buttons.addView(button("Save") { saveCurrent() })
        buttons.addView(button("New") { createProfile() })
        buttons.addView(button("Delete") { deleteCurrent() })
        buttons.addView(button("Layout") { buildEditor() })
        root.addView(buttons)

        root.addView(button("Grant overlay permission") {
            if (!Settings.canDrawOverlays(this)) {
                startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${packageName}")))
            }
        })
        root.addView(button("Open Accessibility settings") {
            startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
        })
        root.addView(button("Start Input Gateway") {
            startServiceCompat(Intent(this, InputGatewayService::class.java))
            setStatus("Gateway listening on 127.0.0.1:${InputGatewayService.PORT}")
        })
        root.addView(button("Show Layout Overlay") {
            if (Settings.canDrawOverlays(this)) {
                startServiceCompat(Intent(this, OverlayService::class.java))
                setStatus("Overlay shown")
            } else setStatus("Grant overlay permission first")
        })

        root.addView(numberRow("Aim sensitivity", .05, 8.0, current.aimSensitivity.toDouble()) {
            current.aimSensitivity = it.toFloat()
        })
        root.addView(CheckBox(this).apply {
            text = "Invert Y"
            setTextColor(Color.WHITE)
            isChecked = current.invertY
            setOnCheckedChangeListener { _, v -> current.invertY = v }
        })

        status = TextView(this).apply {
            text = "Ready"
            setTextColor(Color.LTGRAY)
            setPadding(0, 16, 0, 0)
        }
        root.addView(status)
        setContentView(root)
        refreshProfiles()
    }

    private fun buildEditor() {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.rgb(16, 17, 20))
            setPadding(10, 10, 10, 10)
        }
        val top = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        top.addView(button("Back") { buildMain() })
        top.addView(button("Save") { saveCurrent() })
        top.addView(button("Add") { addBindingDialog() })
        root.addView(top)

        editor = LayoutEditorView(this).apply { profile = current }
        root.addView(editor, LinearLayout.LayoutParams(-1, 0, 1f))
        root.addView(TextView(this).apply {
            text = "Drag markers to reposition controls."
            setTextColor(Color.LTGRAY)
        })
        setContentView(root)
    }

    private fun addBindingDialog() {
        val panel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(16, 4, 16, 4)
        }
        fun field(h: String, v: String) = EditText(this).apply { hint = h; setText(v) }
        val source = field("source: key or mouse", "key")
        val code = field("Linux event code", "57").apply { inputType = InputType.TYPE_CLASS_NUMBER }
        val trigger = field("trigger: TAP or HOLD", "TAP")
        val x = field("x 0..1", "0.5")
        val y = field("y 0..1", "0.5")
        val label = field("label", "KEY")
        listOf(source, code, trigger, x, y, label).forEach(panel::addView)

        AlertDialog.Builder(this)
            .setTitle("Add binding")
            .setView(panel)
            .setPositiveButton("Add") { _, _ ->
                current.bindings += Binding(
                    source.text.toString().trim().ifBlank { "key" },
                    code.text.toString().toIntOrNull() ?: 0,
                    runCatching { Trigger.valueOf(trigger.text.toString().uppercase()) }.getOrDefault(Trigger.TAP),
                    x.text.toString().toFloatOrNull()?.coerceIn(0f, 1f) ?: .5f,
                    y.text.toString().toFloatOrNull()?.coerceIn(0f, 1f) ?: .5f,
                    label.text.toString().ifBlank { "KEY" },
                    current.bindings.size.coerceAtMost(9)
                )
                editor?.profile = current
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun createProfile() {
        val input = EditText(this).apply { hint = "Profile name"; setText("New Profile") }
        AlertDialog.Builder(this)
            .setTitle("Create profile")
            .setView(input)
            .setPositiveButton("Create") { _, _ ->
                current = Profile(input.text.toString().ifBlank { "New Profile" }, bindings = mutableListOf())
                saveCurrent()
                buildMain()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun deleteCurrent() {
        val safe = current.name.replace(Regex("[^a-zA-Z0-9._-]+"), "_")
        val f = java.io.File(filesDir, "profiles/${safe}.json")
        if (f.exists()) f.delete()
        current = Profile.freeFire()
        saveCurrent()
        buildMain()
    }

    private fun refreshProfiles() {
        val names = store.list().ifEmpty { listOf(current.name) }
        val adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names)
        spinner.adapter = adapter
        val idx = names.indexOf(current.name)
        if (idx >= 0) spinner.setSelection(idx)
        spinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onNothingSelected(parent: AdapterView<*>?) = Unit
            override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                store.load(names[position])?.let { current = it }
            }
        }
    }

    private fun numberRow(label: String, min: Double, max: Double, value: Double, changed: (Double) -> Unit): View {
        val row = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        row.addView(TextView(this).apply {
            text = label
            setTextColor(Color.WHITE)
        }, LinearLayout.LayoutParams(0, 60, 1f))
        row.addView(EditText(this).apply {
            inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL
            setText(value.toString())
            setTextColor(Color.WHITE)
            setOnFocusChangeListener { _, hasFocus ->
                if (!hasFocus) changed(text.toString().toDoubleOrNull()?.coerceIn(min, max) ?: value)
            }
        }, LinearLayout.LayoutParams(180, 60))
        return row
    }

    private fun button(text: String, action: () -> Unit) = Button(this).apply {
        this.text = text
        setOnClickListener { action() }
    }

    private fun startServiceCompat(i: Intent) {
        if (android.os.Build.VERSION.SDK_INT >= 26) startForegroundService(i) else startService(i)
    }

    private fun requestNotificationPermission() {
        if (android.os.Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 42)
    }

    private fun saveCurrent() {
        store.save(current)
        InputGatewayService.setActiveProfile(current)
        setStatus("Saved " + current.name)
    }

    private fun setStatus(s: String) { if (::status.isInitialized) status.text = s }
}

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
import kotlin.math.max
import kotlin.math.min

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
        val scroll = ScrollView(this)
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(20, 18, 20, 24)
            setBackgroundColor(Color.rgb(14, 15, 18))
        }

        root.addView(TextView(this).apply {
            text = "WAYDROID KEYMAPPER"
            textSize = 25f
            setTextColor(Color.WHITE)
            setTypeface(typeface, android.graphics.Typeface.BOLD)
        }, LinearLayout.LayoutParams(-1, 64))

        root.addView(TextView(this).apply {
            text = "Laptop keyboard + mouse → ADB transport → Android mapper"
            textSize = 13f
            setTextColor(Color.LTGRAY)
        }, LinearLayout.LayoutParams(-1, 42))

        root.addView(sectionTitle("PROFILE"))
        spinner = Spinner(this)
        root.addView(spinner, LinearLayout.LayoutParams(-1, 58))

        val profileButtons = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        profileButtons.addView(button("Save") { saveCurrent() })
        profileButtons.addView(button("New") { createProfile() })
        profileButtons.addView(button("Delete") { deleteCurrent() })
        profileButtons.addView(button("Mappings") { showMappingsDialog() })
        root.addView(profileButtons)

        root.addView(sectionTitle("INPUT"))
        root.addView(button("Open Accessibility settings") {
            startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
        })
        root.addView(button("Start Input Gateway") { startGateway() })
        root.addView(button("Stop Input Gateway") {
            stopService(Intent(this, InputGatewayService::class.java))
            setStatus("Gateway stopped")
        })

        root.addView(sectionTitle("OVERLAY / LAYOUT"))
        root.addView(button("Grant overlay permission") {
            if (!Settings.canDrawOverlays(this)) {
                startActivity(
                    Intent(
                        Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:$packageName")
                    )
                )
            } else setStatus("Overlay permission already granted")
        })
        root.addView(button("Show Layout Overlay") {
            if (Settings.canDrawOverlays(this)) {
                startService(Intent(this, OverlayService::class.java))
                setStatus("Overlay shown")
            } else setStatus("Grant overlay permission first")
        })
        root.addView(button("Hide Layout Overlay") {
            stopService(Intent(this, OverlayService::class.java))
            setStatus("Overlay hidden")
        })
        root.addView(button("Open Layout Editor") { buildEditor() })

        root.addView(sectionTitle("AIM"))
        root.addView(numberRow("Sensitivity", 0.05, 12.0, current.aimSensitivity.toDouble()) {
            current.aimSensitivity = it.toFloat()
            editor?.invalidate()
        })
        root.addView(numberRow("Aim X", 0.0, 1.0, current.aimX.toDouble()) {
            current.aimX = it.toFloat()
            editor?.invalidate()
        })
        root.addView(numberRow("Aim Y", 0.0, 1.0, current.aimY.toDouble()) {
            current.aimY = it.toFloat()
            editor?.invalidate()
        })
        root.addView(CheckBox(this).apply {
            text = "Enable mouse aim"
            setTextColor(Color.WHITE)
            isChecked = current.aimEnabled
            setOnCheckedChangeListener { _, v -> current.aimEnabled = v }
        })
        root.addView(CheckBox(this).apply {
            text = "Invert Y"
            setTextColor(Color.WHITE)
            isChecked = current.invertY
            setOnCheckedChangeListener { _, v -> current.invertY = v }
        })

        root.addView(sectionTitle("JOYSTICK"))
        root.addView(CheckBox(this).apply {
            text = "Enable joystick"
            setTextColor(Color.WHITE)
            isChecked = current.joystickEnabled
            setOnCheckedChangeListener { _, v -> current.joystickEnabled = v }
        })
        root.addView(numberRow("Joystick X", 0.0, 1.0, current.joystickX.toDouble()) {
            current.joystickX = it.toFloat()
            editor?.invalidate()
        })
        root.addView(numberRow("Joystick Y", 0.0, 1.0, current.joystickY.toDouble()) {
            current.joystickY = it.toFloat()
            editor?.invalidate()
        })
        root.addView(numberRow("Joystick radius", 0.02, 0.25, current.joystickRadius.toDouble()) {
            current.joystickRadius = it.toFloat()
            editor?.invalidate()
        })

        root.addView(sectionTitle("STATUS"))
        status = TextView(this).apply {
            text = "Ready"
            textSize = 13f
            setTextColor(Color.LTGRAY)
            setPadding(0, 8, 0, 8)
        }
        root.addView(status)

        scroll.addView(root)
        setContentView(scroll)
        refreshProfiles()
    }

    private fun buildEditor() {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.rgb(14, 15, 18))
            setPadding(10, 10, 10, 10)
        }

        val top = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        top.addView(button("Back") { buildMain() })
        top.addView(button("Save") { saveCurrent() })
        top.addView(button("Add") { addBindingDialog(null) })
        top.addView(button("Mappings") { showMappingsDialog() })
        root.addView(top)

        editor = LayoutEditorView(this).apply { profile = current }
        root.addView(editor, LinearLayout.LayoutParams(-1, 0, 1f))
        root.addView(TextView(this).apply {
            text = "Drag joystick, aim crosshair, or any mapping marker."
            textSize = 12f
            setTextColor(Color.LTGRAY)
            setPadding(4, 8, 4, 12)
        })

        setContentView(root)
    }

    private fun showMappingsDialog() {
        val panel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(16, 4, 16, 4)
        }

        if (current.bindings.isEmpty()) {
            panel.addView(TextView(this).apply {
                text = "No bindings yet."
                setTextColor(Color.WHITE)
            })
        }

        current.bindings.toList().forEachIndexed { index, binding ->
            val row = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
            row.addView(TextView(this).apply {
                text = "${binding.label}  •  ${binding.source}:${binding.code}  •  ${binding.trigger}"
                setTextColor(Color.WHITE)
            }, LinearLayout.LayoutParams(0, 56, 1f))
            row.addView(button("Edit") { addBindingDialog(index) })
            row.addView(button("X") {
                current.bindings.remove(binding)
                editor?.profile = current
                setStatus("Removed ${binding.label}")
            })
            panel.addView(row)
        }

        AlertDialog.Builder(this)
            .setTitle("Mappings")
            .setView(panel)
            .setPositiveButton("Add") { _, _ -> addBindingDialog(null) }
            .setNegativeButton("Close", null)
            .show()
    }

    private fun addBindingDialog(editIndex: Int?) {
        val panel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(16, 4, 16, 4)
        }

        val existing = editIndex?.let { current.bindings.getOrNull(it) }
        fun field(h: String, v: String) = EditText(this).apply {
            hint = h
            setText(v)
            setTextColor(Color.WHITE)
        }

        val source = field("source: key or mouse", existing?.source ?: "key")
        val code = field("Linux event code", existing?.code?.toString() ?: "57").apply {
            inputType = InputType.TYPE_CLASS_NUMBER
        }
        val trigger = field("trigger: TAP or HOLD", existing?.trigger?.name ?: "TAP")
        val x = field("x 0..1", existing?.x?.toString() ?: "0.5")
        val y = field("y 0..1", existing?.y?.toString() ?: "0.5")
        val label = field("label", existing?.label ?: "KEY")

        listOf(source, code, trigger, x, y, label).forEach(panel::addView)

        AlertDialog.Builder(this)
            .setTitle(if (editIndex == null) "Add binding" else "Edit binding")
            .setView(panel)
            .setPositiveButton(if (editIndex == null) "Add" else "Save") { _, _ ->
                val normalizedSource =
                    source.text.toString().trim().lowercase().let { if (it == "mouse") "mouse" else "key" }

                val value = Binding(
                    normalizedSource,
                    code.text.toString().toIntOrNull() ?: 0,
                    runCatching {
                        Trigger.valueOf(trigger.text.toString().trim().uppercase())
                    }.getOrDefault(Trigger.TAP),
                    x.text.toString().toFloatOrNull()?.coerceIn(0f, 1f) ?: .5f,
                    y.text.toString().toFloatOrNull()?.coerceIn(0f, 1f) ?: .5f,
                    label.text.toString().trim().ifBlank { "KEY" },
                    existing?.slot ?: current.bindings.size.coerceIn(0, 9)
                )

                if (editIndex == null) current.bindings += value
                else current.bindings[editIndex] = value

                editor?.profile = current
                setStatus("Mapping saved")
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun createProfile() {
        val input = EditText(this).apply {
            hint = "Profile name"
            setText("New Profile")
            setTextColor(Color.WHITE)
        }

        AlertDialog.Builder(this)
            .setTitle("Create profile")
            .setView(input)
            .setPositiveButton("Create") { _, _ ->
                current = Profile(
                    input.text.toString().ifBlank { "New Profile" },
                    bindings = mutableListOf()
                )
                saveCurrent()
                buildMain()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun deleteCurrent() {
        val name = current.name
        store.delete(name)
        current = store.active()
        setStatus("Deleted $name")
        buildMain()
    }

    private fun refreshProfiles() {
        val names = store.list().ifEmpty {
            store.save(Profile.freeFire())
            listOf(Profile.freeFire().name)
        }

        val adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names)
        spinner.adapter = adapter
        val idx = names.indexOf(current.name)
        if (idx >= 0) spinner.setSelection(idx)

        spinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onNothingSelected(parent: AdapterView<*>?) = Unit

            override fun onItemSelected(
                parent: AdapterView<*>?, view: View?, position: Int, id: Long
            ) {
                store.load(names[position])?.let {
                    current = it
                    InputGatewayService.setActiveProfile(it)
                    editor?.profile = it
                }
            }
        }
    }

    private fun startGateway() {
        runCatching {
            if (android.os.Build.VERSION.SDK_INT >= 26) {
                startForegroundService(Intent(this, InputGatewayService::class.java))
            } else {
                startService(Intent(this, InputGatewayService::class.java))
            }
            InputGatewayService.setActiveProfile(current)
            setStatus("Gateway listening on 127.0.0.1:${InputGatewayService.PORT}")
        }.onFailure {
            setStatus("Gateway start failed: ${it.message}")
        }
    }

    private fun numberRow(
        label: String,
        minValue: Double,
        maxValue: Double,
        value: Double,
        changed: (Double) -> Unit
    ): View {
        val row = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }

        row.addView(TextView(this).apply {
            text = label
            setTextColor(Color.WHITE)
        }, LinearLayout.LayoutParams(0, 58, 1f))

        row.addView(EditText(this).apply {
            inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL
            setText(value.toString())
            setTextColor(Color.WHITE)
            setSingleLine(true)
            setOnFocusChangeListener { _, hasFocus ->
                if (!hasFocus) {
                    changed(
                        text.toString().toDoubleOrNull()
                            ?.coerceIn(min(minValue, maxValue), max(minValue, maxValue))
                            ?: value
                    )
                }
            }
        }, LinearLayout.LayoutParams(180, 58))

        return row
    }

    private fun sectionTitle(text: String): TextView = TextView(this).apply {
        this.text = text
        textSize = 12f
        setTextColor(Color.rgb(255, 80, 95))
        setTypeface(typeface, android.graphics.Typeface.BOLD)
        setPadding(0, 18, 0, 6)
    }

    private fun button(text: String, action: () -> Unit) = Button(this).apply {
        this.text = text
        minWidth = 0
        setOnClickListener { action() }
    }

    private fun requestNotificationPermission() {
        if (
            android.os.Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 42)
        }
    }

    private fun saveCurrent() {
        store.save(current)
        InputGatewayService.setActiveProfile(current)
        setStatus("Saved ${current.name}")
    }

    private fun setStatus(s: String) {
        if (::status.isInitialized) status.text = s
    }
}

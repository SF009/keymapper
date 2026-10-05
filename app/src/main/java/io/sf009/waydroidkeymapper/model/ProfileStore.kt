package io.sf009.waydroidkeymapper.model

import android.content.Context
import java.io.File

class ProfileStore(private val context: Context) {
    private val dir = File(context.filesDir, "profiles")
    private val activeFile = File(dir, "active.json")

    init {
        dir.mkdirs()
        if (!activeFile.exists()) {
            save(Profile.freeFire())
            writeProfile(Profile.minimal())
        }
    }

    fun list(): List<String> =
        dir.listFiles { f ->
            f.isFile && f.extension == "json" && f.name != activeFile.name
        }?.map { it.nameWithoutExtension }?.sorted() ?: emptyList()

    fun save(profile: Profile) {
        dir.mkdirs()
        writeProfile(profile)
        activeFile.writeText(profile.toJson().toString())
    }

    fun load(name: String): Profile? {
        val f = File(dir, "${safe(name)}.json")
        return if (f.exists()) {
            runCatching {
                Profile.fromJson(org.json.JSONObject(f.readText()))
            }.getOrNull()
        } else null
    }

    fun delete(name: String) {
        val f = File(dir, "${safe(name)}.json")
        if (f.exists()) f.delete()

        val remaining = list()
        val next = remaining.firstOrNull()?.let { load(it) } ?: Profile.freeFire()
        save(next)
    }

    fun active(): Profile =
        runCatching {
            Profile.fromJson(org.json.JSONObject(activeFile.readText()))
        }.getOrElse { Profile.freeFire() }

    private fun writeProfile(profile: Profile) {
        File(dir, "${safe(profile.name)}.json").writeText(profile.toJson().toString(2))
    }

    private fun safe(name: String): String =
        name.trim()
            .replace(Regex("[^a-zA-Z0-9._-]+"), "_")
            .ifBlank { "profile" }
}

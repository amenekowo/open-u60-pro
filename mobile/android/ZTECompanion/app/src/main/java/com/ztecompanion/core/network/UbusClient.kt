package com.ztecompanion.core.network

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class UbusClient @Inject constructor() {

    companion object {
        const val ANON_SESSION = "00000000000000000000000000000000"
        private val JSON_MEDIA_TYPE = "application/json".toMediaType()
    }

    private val idCounter = AtomicInteger(0)

    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(15, TimeUnit.SECONDS)
        .writeTimeout(10, TimeUnit.SECONDS)
        .build()

    var gateway: String = "192.168.0.1"
    var session: String = ANON_SESSION
        private set

    val isAuthenticated: Boolean get() = session != ANON_SESSION

    private val baseUrl: String get() = "http://$gateway/ubus/"

    fun updateGateway(newGateway: String) {
        gateway = newGateway
    }

    fun setSession(newSession: String) {
        session = newSession
    }

    fun clearSession() {
        session = ANON_SESSION
    }

    private fun nextId(): Int = idCounter.incrementAndGet()

    private fun timestampMs(): Long = System.currentTimeMillis()

    private suspend fun post(payload: JsonArray): JsonArray = withContext(Dispatchers.IO) {
        val url = "$baseUrl?t=${timestampMs()}"
        val body = payload.toString().toRequestBody(JSON_MEDIA_TYPE)
        val request = Request.Builder()
            .url(url)
            .post(body)
            .header("Content-Type", "application/json")
            .build()

        try {
            val response = httpClient.newCall(request).execute()
            if (!response.isSuccessful) {
                throw UbusError.NetworkError("HTTP ${response.code}: ${response.message}")
            }
            val responseBody = response.body?.string()
                ?: throw UbusError.ParseError("Empty response body")
            val element = Json.parseToJsonElement(responseBody)
            when (element) {
                is JsonArray -> element
                is JsonObject -> buildJsonArray { add(element) }
                else -> throw UbusError.ParseError("Unexpected response type")
            }
        } catch (e: UbusError) {
            throw e
        } catch (e: java.net.SocketTimeoutException) {
            throw UbusError.TimeoutError("Connection timed out to $gateway")
        } catch (e: java.net.ConnectException) {
            throw UbusError.NetworkError("Cannot connect to $gateway", e)
        } catch (e: Exception) {
            throw UbusError.NetworkError("Network error: ${e.message}", e)
        }
    }

    private suspend fun rpc(ubusMethod: String, params: JsonArray): JsonElement? {
        val rpcPayload = buildJsonObject {
            put("jsonrpc", "2.0")
            put("id", nextId())
            put("method", ubusMethod)
            put("params", params)
        }
        val wrapped = buildJsonArray { add(rpcPayload) }
        val results = post(wrapped)
        if (results.isEmpty()) throw UbusError.ParseError("Empty response from ubus")
        val result = results[0].jsonObject
        val error = result["error"]
        if (error != null) {
            throw UbusError.CallError(0, "ubus error: $error")
        }
        return result["result"]
    }

    suspend fun getSalt(retries: Int = 3): String {
        var lastError: Exception? = null
        repeat(retries) { attempt ->
            try {
                val params = buildJsonArray {
                    add(ANON_SESSION)
                    add("zwrt_web")
                    add("web_login_info")
                    add(buildJsonObject {})
                }
                val result = rpc("call", params)
                if (result is JsonArray && result.size >= 2) {
                    val info = result[1]
                    if (info is JsonObject) {
                        val salt = info["zte_web_sault"]?.jsonPrimitive?.contentOrNull
                            ?: info["salt"]?.jsonPrimitive?.contentOrNull
                        if (!salt.isNullOrEmpty()) return salt
                    }
                }
                lastError = UbusError.AuthError("No salt in response (attempt ${attempt + 1})")
            } catch (e: Exception) {
                lastError = e
            }
            if (attempt < retries - 1) {
                kotlinx.coroutines.delay(500)
            }
        }
        throw UbusError.AuthError("Failed to fetch salt after $retries attempts: ${lastError?.message}")
    }

    fun hashPassword(password: String, salt: String): String {
        val sha256 = MessageDigest.getInstance("SHA-256")
        val firstHash = sha256.digest(password.toByteArray()).toHexString().uppercase()
        sha256.reset()
        val combined = firstHash + salt
        return sha256.digest(combined.toByteArray()).toHexString().uppercase()
    }

    suspend fun login(password: String): String {
        val salt = getSalt()
        val hashed = hashPassword(password, salt)
        val params = buildJsonArray {
            add(ANON_SESSION)
            add("zwrt_web")
            add("web_login")
            add(buildJsonObject { put("password", hashed) })
        }
        val result = rpc("call", params)
        if (result is JsonArray && result.size >= 2) {
            val info = result[1]
            if (info is JsonObject) {
                val newSession = info["ubus_rpc_session"]?.jsonPrimitive?.contentOrNull
                if (!newSession.isNullOrEmpty() && newSession != ANON_SESSION) {
                    session = newSession
                    return newSession
                }
            }
        }
        throw UbusError.AuthError("Login failed: invalid session returned")
    }

    suspend fun call(obj: String, method: String, callParams: JsonObject = buildJsonObject {}): JsonObject? {
        if (session == ANON_SESSION) {
            throw UbusError.AuthError("Not logged in")
        }
        val params = buildJsonArray {
            add(session)
            add(obj)
            add(method)
            add(callParams)
        }
        val result = rpc("call", params)
        if (result is JsonArray) {
            if (result.isNotEmpty()) {
                val code = result[0].jsonPrimitive.intOrNull ?: -1
                if (code != 0) {
                    throw UbusError.CallError(code, "ubus call failed (code $code): $obj.$method")
                }
            }
            if (result.size >= 2) {
                return result[1].jsonObject
            }
            return null
        }
        return null
    }

    suspend fun callAnon(obj: String, method: String, callParams: JsonObject = buildJsonObject {}): JsonObject? {
        val params = buildJsonArray {
            add(ANON_SESSION)
            add(obj)
            add(method)
            add(callParams)
        }
        val result = rpc("call", params)
        if (result is JsonArray && result.size >= 2) {
            return result[1].jsonObject
        }
        return null
    }

    private fun ByteArray.toHexString(): String =
        joinToString("") { "%02x".format(it) }
}

package com.ztecompanion.core.network

sealed class UbusError : Exception() {
    data class NetworkError(override val message: String, override val cause: Throwable? = null) : UbusError()
    data class AuthError(override val message: String) : UbusError()
    data class CallError(val code: Int, override val message: String) : UbusError()
    data class ParseError(override val message: String) : UbusError()
    data class TimeoutError(override val message: String) : UbusError()
}

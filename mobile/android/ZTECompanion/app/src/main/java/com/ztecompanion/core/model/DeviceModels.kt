package com.ztecompanion.core.model

import kotlinx.serialization.Serializable

@Serializable
data class BatteryStatus(
    val capacity: Int = 0,
    val temperature: Int = 0,
)

@Serializable
data class ThermalStatus(
    val cpuTemp: Int = 0,
)

data class TrafficStats(
    val rxBytes: Long = 0,
    val txBytes: Long = 0,
    val rxBytesPerSec: Double = 0.0,
    val txBytesPerSec: Double = 0.0,
    val timestamp: Long = System.currentTimeMillis(),
)

fun formatSpeed(bytesPerSec: Double): String {
    val bits = bytesPerSec * 8.0
    val tb = 1024.0 * 1024.0 * 1024.0 * 1024.0
    val gb = 1024.0 * 1024.0 * 1024.0
    val mb = 1024.0 * 1024.0
    val kb = 1024.0
    val round2 = { v: Double -> Math.round(v * 100.0) / 100.0 }
    return when {
        bits / tb >= 0.5 -> "%.2fTb/s".format(round2(bits / tb))
        bits / gb >= 0.5 -> "%.2fGb/s".format(round2(bits / gb))
        bits / mb >= 0.5 -> "%.2fMb/s".format(round2(bits / mb))
        bits / kb >= 0.5 -> "%.2fKb/s".format(round2(bits / kb))
        else -> "%.2fb/s".format(round2(bits))
    }
}

fun formatBytes(bytes: Long): String {
    val b = bytes.toDouble()
    val tb = 1024.0 * 1024.0 * 1024.0 * 1024.0
    val gb = 1024.0 * 1024.0 * 1024.0
    val mb = 1024.0 * 1024.0
    val kb = 1024.0
    val round2 = { v: Double -> Math.round(v * 100.0) / 100.0 }
    return when {
        b / tb >= 0.5 -> "%.2fTB".format(round2(b / tb))
        b / gb >= 0.5 -> "%.2fGB".format(round2(b / gb))
        b / mb >= 0.5 -> "%.2fMB".format(round2(b / mb))
        b / kb >= 0.5 -> "%.2fKB".format(round2(b / kb))
        else -> "${bytes}B"
    }
}

@Serializable
data class SimInfo(
    val iccid: String = "",
    val imsi: String = "",
    val msisdn: String = "",
)

@Serializable
data class DeviceInfo(
    val imei: String = "",
    val sim: SimInfo = SimInfo(),
    val wanIpv4: String = "",
    val wanIpv6: String = "",
    val lanIp: String = "",
)

@Serializable
data class ConnectedClient(
    val mac: String = "",
    val name: String = "",
    val ip: String = "",
    val ip6: String = "",
    val hostname: String = "",
)

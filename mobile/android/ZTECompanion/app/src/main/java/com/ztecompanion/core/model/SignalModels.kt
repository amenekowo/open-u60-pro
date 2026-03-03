package com.ztecompanion.core.model

import kotlinx.serialization.Serializable

@Serializable
data class NRSignal(
    val rsrp: Int? = null,
    val rsrq: Int? = null,
    val sinr: Int? = null,
    val rssi: Int? = null,
    val band: String = "",
    val pci: String = "",
    val cellId: String = "",
    val arfcn: String = "",
    val bandwidth: String = "",
    val ca: String = "",
)

@Serializable
data class LTESignal(
    val rsrp: Int? = null,
    val rsrq: Int? = null,
    val sinr: Int? = null,
    val rssi: Int? = null,
    val ca: String = "",
    val caState: String = "",
    val caSig: String = "",
)

@Serializable
data class WCDMASignal(
    val rscp: Int? = null,
    val ecio: Int? = null,
)

@Serializable
data class OperatorInfo(
    val provider: String = "",
    val networkType: String = "",
    val signalBar: Int = 0,
    val roaming: String = "",
)

data class SignalSnapshot(
    val nr: NRSignal = NRSignal(),
    val lte: LTESignal = LTESignal(),
    val wcdma: WCDMASignal = WCDMASignal(),
    val operator: OperatorInfo = OperatorInfo(),
    val timestamp: Long = System.currentTimeMillis(),
)

package com.ztecompanion.feature.dashboard

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ztecompanion.core.model.*
import com.ztecompanion.core.network.AuthManager
import com.ztecompanion.core.network.AuthState
import com.ztecompanion.core.network.UbusClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import javax.inject.Inject

data class DashboardState(
    val signal: SignalSnapshot = SignalSnapshot(),
    val battery: BatteryStatus = BatteryStatus(),
    val thermal: ThermalStatus = ThermalStatus(),
    val traffic: TrafficStats = TrafficStats(),
    val clientCount: Int = 0,
    val isLoading: Boolean = false,
    val error: String? = null,
)

@HiltViewModel
class DashboardViewModel @Inject constructor(
    private val ubusClient: UbusClient,
    private val authManager: AuthManager,
) : ViewModel() {

    private val _state = MutableStateFlow(DashboardState())
    val state: StateFlow<DashboardState> = _state.asStateFlow()

    val authState = authManager.authState

    private var pollingJob: Job? = null
    private var prevRxBytes: Long = 0
    private var prevTxBytes: Long = 0
    private var prevTimestamp: Long = 0
    private var prevSource: String = ""

    init {
        viewModelScope.launch {
            authManager.authState.collect { authState ->
                if (authState == AuthState.LOGGED_IN) {
                    startPolling()
                } else {
                    stopPolling()
                }
            }
        }
        viewModelScope.launch {
            authManager.autoLogin()
        }
    }

    fun refresh() {
        viewModelScope.launch { fetchAll() }
    }

    private fun startPolling() {
        pollingJob?.cancel()
        pollingJob = viewModelScope.launch {
            while (isActive) {
                fetchAll()
                delay(authManager.pollInterval * 1000L)
            }
        }
    }

    private fun stopPolling() {
        pollingJob?.cancel()
        pollingJob = null
    }

    private suspend fun fetchAll() {
        _state.value = _state.value.copy(isLoading = true, error = null)
        try {
            val signal = fetchSignal()
            val battery = fetchBattery()
            val thermal = fetchThermal()
            val traffic = fetchTraffic()
            val clientCount = fetchClientCount()
            _state.value = _state.value.copy(
                signal = signal,
                battery = battery,
                thermal = thermal,
                traffic = traffic,
                clientCount = clientCount,
                isLoading = false,
            )
        } catch (e: Exception) {
            _state.value = _state.value.copy(
                isLoading = false,
                error = e.message ?: "Unknown error",
            )
        }
    }

    private suspend fun fetchSignal(): SignalSnapshot {
        val data = ubusClient.call("zte_nwinfo_api", "nwinfo_get_netinfo") ?: return SignalSnapshot()
        return SignalSnapshot(
            nr = NRSignal(
                rsrp = data["nr5g_rsrp"]?.jsonPrimitive?.intOrNull,
                rsrq = data["nr5g_rsrq"]?.jsonPrimitive?.intOrNull,
                sinr = data["nr5g_snr"]?.jsonPrimitive?.intOrNull,
                rssi = data["nr5g_rssi"]?.jsonPrimitive?.intOrNull,
                band = data["nr5g_action_band"]?.jsonPrimitive?.contentOrNull ?: "",
                pci = data["nr5g_pci"]?.jsonPrimitive?.contentOrNull ?: "",
                cellId = data["nr5g_cell_id"]?.jsonPrimitive?.contentOrNull ?: "",
                arfcn = data["nr5g_action_channel"]?.jsonPrimitive?.contentOrNull ?: "",
                bandwidth = data["nr5g_bandwidth"]?.jsonPrimitive?.contentOrNull ?: "",
                ca = data["nrca"]?.jsonPrimitive?.contentOrNull ?: "",
            ),
            lte = LTESignal(
                rsrp = data["lte_rsrp"]?.jsonPrimitive?.intOrNull,
                rsrq = data["lte_rsrq"]?.jsonPrimitive?.intOrNull,
                sinr = data["lte_snr"]?.jsonPrimitive?.intOrNull,
                rssi = data["lte_rssi"]?.jsonPrimitive?.intOrNull,
                ca = data["lteca"]?.jsonPrimitive?.contentOrNull ?: "",
                caState = data["lteca_state"]?.jsonPrimitive?.contentOrNull ?: "",
                caSig = data["ltecasig"]?.jsonPrimitive?.contentOrNull ?: "",
            ),
            wcdma = WCDMASignal(
                rscp = data["rscp"]?.jsonPrimitive?.intOrNull,
                ecio = data["ecio"]?.jsonPrimitive?.intOrNull,
            ),
            operator = OperatorInfo(
                provider = data["network_provider"]?.jsonPrimitive?.contentOrNull ?: "",
                networkType = data["network_type"]?.jsonPrimitive?.contentOrNull ?: "",
                signalBar = data["signalbar"]?.jsonPrimitive?.intOrNull ?: 0,
                roaming = data["simcard_roam"]?.jsonPrimitive?.contentOrNull ?: "",
            ),
        )
    }

    private suspend fun fetchBattery(): BatteryStatus {
        val data = ubusClient.call("zwrt_bsp.battery", "list") ?: return BatteryStatus()
        return BatteryStatus(
            capacity = data["battery_capacity"]?.jsonPrimitive?.intOrNull ?: 0,
            temperature = data["battery_temperature"]?.jsonPrimitive?.intOrNull ?: 0,
        )
    }

    private suspend fun fetchThermal(): ThermalStatus {
        val data = ubusClient.call("zwrt_bsp.thermal", "get_cpu_temp") ?: return ThermalStatus()
        return ThermalStatus(
            cpuTemp = data["cpuss_temp"]?.jsonPrimitive?.intOrNull ?: 0,
        )
    }

    private suspend fun fetchTraffic(): TrafficStats {
        var rxBytes: Long = 0
        var txBytes: Long = 0
        var source = ""
        var precomputedRxRate: Double? = null
        var precomputedTxRate: Double? = null

        // Primary: zwrt_data get_wwandst (modem-level counters, matches router web UI)
        val wwandst = try { ubusClient.call("zwrt_data", "get_wwandst") } catch (_: Exception) { null }
        if (wwandst != null) {
            val rx = wwandst["real_rx_bytes"]?.jsonPrimitive?.longOrNull
            if (rx != null) {
                rxBytes = rx
                txBytes = wwandst["real_tx_bytes"]?.jsonPrimitive?.longOrNull ?: 0
                source = "wwandst"
                precomputedRxRate = wwandst["real_rx_speed"]?.jsonPrimitive?.doubleOrNull
                precomputedTxRate = wwandst["real_tx_speed"]?.jsonPrimitive?.doubleOrNull
            }
        }

        // Fallback: network.device status (rmnet_data0)
        if (source.isEmpty()) {
            val params = buildJsonObject { put("name", "rmnet_data0") }
            val data = try { ubusClient.call("network.device", "status", params) } catch (_: Exception) { null }
            val stats = data?.get("statistics")?.jsonObject
            if (stats != null) {
                rxBytes = stats["rx_bytes"]?.jsonPrimitive?.longOrNull ?: 0
                txBytes = stats["tx_bytes"]?.jsonPrimitive?.longOrNull ?: 0
                if (rxBytes > 0) source = "rmnet_ubus"
            }
        }

        if (source.isEmpty()) return TrafficStats()

        val now = System.currentTimeMillis()
        var rxSpeed = 0.0
        var txSpeed = 0.0

        if (precomputedRxRate != null && precomputedTxRate != null) {
            // Use pre-computed rates (bytes/sec) from router
            rxSpeed = precomputedRxRate
            txSpeed = precomputedTxRate
        } else if (prevTimestamp > 0 && now > prevTimestamp && source == prevSource) {
            // Delta computation (bytes/sec), skip when source changes to avoid invalid spikes
            val dtSec = (now - prevTimestamp) / 1000.0
            if (dtSec > 0) {
                rxSpeed = (rxBytes - prevRxBytes) / dtSec
                txSpeed = (txBytes - prevTxBytes) / dtSec
                if (rxSpeed < 0) rxSpeed = 0.0
                if (txSpeed < 0) txSpeed = 0.0
            }
        }
        prevRxBytes = rxBytes
        prevTxBytes = txBytes
        prevTimestamp = now
        prevSource = source

        return TrafficStats(
            rxBytes = rxBytes,
            txBytes = txBytes,
            rxBytesPerSec = rxSpeed,
            txBytesPerSec = txSpeed,
            timestamp = now,
        )
    }

    private suspend fun fetchClientCount(): Int {
        return try {
            val data = ubusClient.call("luci-rpc", "getHostHints") ?: return 0
            data.size
        } catch (_: Exception) {
            0
        }
    }

    override fun onCleared() {
        super.onCleared()
        stopPolling()
    }
}

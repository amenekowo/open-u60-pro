package com.ztecompanion.feature.signal

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

data class SignalMonitorState(
    val current: SignalSnapshot = SignalSnapshot(),
    val history: List<SignalSnapshot> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
)

@HiltViewModel
class SignalMonitorViewModel @Inject constructor(
    private val ubusClient: UbusClient,
    private val authManager: AuthManager,
) : ViewModel() {

    private val _state = MutableStateFlow(SignalMonitorState())
    val state: StateFlow<SignalMonitorState> = _state.asStateFlow()

    val authState = authManager.authState

    private var pollingJob: Job? = null
    private val maxHistory = 60

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
    }

    fun refresh() {
        viewModelScope.launch { fetchSignal() }
    }

    private fun startPolling() {
        pollingJob?.cancel()
        pollingJob = viewModelScope.launch {
            while (isActive) {
                fetchSignal()
                delay(authManager.pollInterval * 1000L)
            }
        }
    }

    private fun stopPolling() {
        pollingJob?.cancel()
        pollingJob = null
    }

    private suspend fun fetchSignal() {
        _state.value = _state.value.copy(isLoading = true, error = null)
        try {
            val data = ubusClient.call("zte_nwinfo_api", "nwinfo_get_netinfo")
            if (data != null) {
                val snapshot = SignalSnapshot(
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
                val newHistory = (_state.value.history + snapshot).takeLast(maxHistory)
                _state.value = _state.value.copy(
                    current = snapshot,
                    history = newHistory,
                    isLoading = false,
                )
            }
        } catch (e: Exception) {
            _state.value = _state.value.copy(
                isLoading = false,
                error = e.message,
            )
        }
    }

    override fun onCleared() {
        super.onCleared()
        stopPolling()
    }
}

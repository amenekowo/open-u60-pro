package com.ztecompanion.feature.deviceinfo

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ztecompanion.core.model.DeviceInfo
import com.ztecompanion.core.model.SimInfo
import com.ztecompanion.core.network.UbusClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import javax.inject.Inject

data class DeviceInfoState(
    val info: DeviceInfo = DeviceInfo(),
    val isLoading: Boolean = false,
    val error: String? = null,
)

@HiltViewModel
class DeviceInfoViewModel @Inject constructor(
    private val ubusClient: UbusClient,
) : ViewModel() {

    private val _state = MutableStateFlow(DeviceInfoState())
    val state: StateFlow<DeviceInfoState> = _state.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            try {
                val sim = fetchSimInfo()
                val imei = fetchImei()
                val wanIpv4 = fetchWanIpv4()
                val wanIpv6 = fetchWanIpv6()
                val lanIp = fetchLanIp()
                _state.value = _state.value.copy(
                    info = DeviceInfo(
                        imei = imei,
                        sim = sim,
                        wanIpv4 = wanIpv4,
                        wanIpv6 = wanIpv6,
                        lanIp = lanIp,
                    ),
                    isLoading = false,
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
            }
        }
    }

    private suspend fun fetchSimInfo(): SimInfo {
        val data = ubusClient.call("zwrt_zte_mdm.api", "get_sim_info") ?: return SimInfo()
        return SimInfo(
            iccid = data["sim_iccid"]?.jsonPrimitive?.contentOrNull ?: "",
            imsi = data["sim_imsi"]?.jsonPrimitive?.contentOrNull ?: "",
            msisdn = data["msisdn"]?.jsonPrimitive?.contentOrNull ?: "",
        )
    }

    private suspend fun fetchImei(): String {
        val data = ubusClient.call("zwrt_zte_mdm.api", "get_imei") ?: return ""
        return data["imei"]?.jsonPrimitive?.contentOrNull ?: ""
    }

    private suspend fun fetchWanIpv4(): String {
        val data = ubusClient.call("network.interface.zte_wan", "status") ?: return ""
        val addrs = data["ipv4-address"]?.jsonArray ?: return ""
        if (addrs.isEmpty()) return ""
        return addrs[0].jsonObject["address"]?.jsonPrimitive?.contentOrNull ?: ""
    }

    private suspend fun fetchWanIpv6(): String {
        val data = ubusClient.call("network.interface.zte_wan6", "status") ?: return ""
        val addrs = data["ipv6-address"]?.jsonArray ?: return ""
        for (addr in addrs) {
            val ip = addr.jsonObject["address"]?.jsonPrimitive?.contentOrNull ?: continue
            if (!ip.startsWith("fe80")) return ip
        }
        return ""
    }

    private suspend fun fetchLanIp(): String {
        val data = ubusClient.call("network.interface.lan", "status") ?: return ""
        val addrs = data["ipv4-address"]?.jsonArray ?: return ""
        if (addrs.isEmpty()) return ""
        return addrs[0].jsonObject["address"]?.jsonPrimitive?.contentOrNull ?: ""
    }
}

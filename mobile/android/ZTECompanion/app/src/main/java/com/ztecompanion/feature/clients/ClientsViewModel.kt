package com.ztecompanion.feature.clients

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ztecompanion.core.model.ConnectedClient
import com.ztecompanion.core.network.UbusClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import javax.inject.Inject

data class ClientsState(
    val clients: List<ConnectedClient> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
)

@HiltViewModel
class ClientsViewModel @Inject constructor(
    private val ubusClient: UbusClient,
) : ViewModel() {

    private val _state = MutableStateFlow(ClientsState())
    val state: StateFlow<ClientsState> = _state.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            try {
                val clients = fetchClients()
                _state.value = _state.value.copy(clients = clients, isLoading = false)
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
            }
        }
    }

    private suspend fun fetchClients(): List<ConnectedClient> {
        val hints = ubusClient.call("luci-rpc", "getHostHints") ?: return emptyList()

        // Enrich with DHCP leases
        val dhcpLeases = mutableMapOf<String, String>()
        try {
            val leaseParams = buildJsonObject { put("family", 4) }
            val leaseData = ubusClient.call("luci-rpc", "getDHCPLeases", leaseParams)
            val leases = leaseData?.get("dhcp_leases")?.jsonArray
            leases?.forEach { lease ->
                val obj = lease.jsonObject
                val mac = obj["macaddr"]?.jsonPrimitive?.contentOrNull?.lowercase() ?: return@forEach
                val hostname = obj["hostname"]?.jsonPrimitive?.contentOrNull ?: ""
                dhcpLeases[mac] = hostname
            }
        } catch (_: Exception) {}

        return hints.entries.map { (mac, value) ->
            val obj = value.jsonObject
            val name = obj["name"]?.jsonPrimitive?.contentOrNull ?: ""
            val ipAddrs = obj["ipaddrs"]?.jsonArray
            val ip = ipAddrs?.firstOrNull()?.jsonPrimitive?.contentOrNull ?: ""
            val ip6Addrs = obj["ip6addrs"]?.jsonArray
            val ip6 = ip6Addrs?.firstOrNull()?.jsonPrimitive?.contentOrNull ?: ""
            val hostname = dhcpLeases[mac.lowercase()] ?: ""
            ConnectedClient(
                mac = mac,
                name = name,
                ip = ip,
                ip6 = ip6,
                hostname = hostname,
            )
        }.sortedBy { it.ip }
    }
}

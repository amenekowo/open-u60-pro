package com.ztecompanion.feature.bandlock

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ztecompanion.core.model.BandConfig
import com.ztecompanion.core.network.UbusClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import javax.inject.Inject

data class BandLockState(
    val selectedNRBands: Set<Int> = emptySet(),
    val selectedLTEBands: Set<Int> = emptySet(),
    val isLoading: Boolean = false,
    val error: String? = null,
    val successMessage: String? = null,
)

@HiltViewModel
class BandLockViewModel @Inject constructor(
    private val ubusClient: UbusClient,
) : ViewModel() {

    private val _state = MutableStateFlow(BandLockState())
    val state: StateFlow<BandLockState> = _state.asStateFlow()

    fun toggleNRBand(band: Int) {
        val current = _state.value.selectedNRBands.toMutableSet()
        if (band in current) current.remove(band) else current.add(band)
        _state.value = _state.value.copy(selectedNRBands = current, successMessage = null)
    }

    fun toggleLTEBand(band: Int) {
        val current = _state.value.selectedLTEBands.toMutableSet()
        if (band in current) current.remove(band) else current.add(band)
        _state.value = _state.value.copy(selectedLTEBands = current, successMessage = null)
    }

    fun applyNRLock() {
        val bands = _state.value.selectedNRBands.sorted().joinToString(",")
        if (bands.isBlank()) return
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, successMessage = null)
            try {
                // Lock NSA bands
                val nsaParams = buildJsonObject {
                    put("nr5g_type", "nsa")
                    put("nr5g_band", bands)
                }
                ubusClient.call("zte_nwinfo_api", "nwinfo_set_nrbandlock", nsaParams)
                // Lock SA bands
                val saParams = buildJsonObject {
                    put("nr5g_type", "sa")
                    put("nr5g_band", bands)
                }
                ubusClient.call("zte_nwinfo_api", "nwinfo_set_nrbandlock", saParams)
                _state.value = _state.value.copy(
                    isLoading = false,
                    successMessage = "NR bands locked to: $bands",
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
            }
        }
    }

    fun applyLTELock() {
        val bands = _state.value.selectedLTEBands.sorted().joinToString(",")
        if (bands.isBlank()) return
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, successMessage = null)
            try {
                val params = buildJsonObject {
                    put("is_lte_band", "1")
                    put("lte_band_mask", bands)
                    put("is_gw_band", "0")
                    put("gw_band_mask", "")
                }
                ubusClient.call("zte_nwinfo_api", "nwinfo_set_gwl_bandlock", params)
                _state.value = _state.value.copy(
                    isLoading = false,
                    successMessage = "LTE bands locked to: $bands",
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
            }
        }
    }

    fun unlockAll() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, successMessage = null)
            try {
                ubusClient.call("zte_nwinfo_api", "nwinfo_rest_band_rat")
                _state.value = _state.value.copy(
                    isLoading = false,
                    selectedNRBands = emptySet(),
                    selectedLTEBands = emptySet(),
                    successMessage = "All bands unlocked",
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
            }
        }
    }
}

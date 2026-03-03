package com.ztecompanion.feature.signal

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.ztecompanion.core.network.AuthState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SignalMonitorScreen(
    viewModel: SignalMonitorViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()
    val authState by viewModel.authState.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Signal Monitor") })
        },
    ) { padding ->
        if (authState != AuthState.LOGGED_IN) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Text("Login required to monitor signal")
            }
            return@Scaffold
        }

        PullToRefreshBox(
            isRefreshing = state.isLoading,
            onRefresh = { viewModel.refresh() },
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (state.error != null) {
                    Card(
                        colors = CardDefaults.cardColors(
                            containerColor = MaterialTheme.colorScheme.errorContainer,
                        ),
                    ) {
                        Text(
                            state.error!!,
                            modifier = Modifier.padding(16.dp),
                            color = MaterialTheme.colorScheme.onErrorContainer,
                        )
                    }
                }

                // Operator info bar
                val op = state.current.operator
                if (op.provider.isNotBlank()) {
                    Text(
                        "${op.provider} | ${op.networkType} | Signal: ${op.signalBar}/5",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                // NR Signal Panel
                val nr = state.current.nr
                SignalPanel(
                    title = "5G NR",
                    rows = listOf(
                        SignalRow("RSRP", nr.rsrp?.let { "$it dBm" } ?: "--", rsrpColor(nr.rsrp)),
                        SignalRow("RSRQ", nr.rsrq?.let { "$it dB" } ?: "--"),
                        SignalRow("SINR", nr.sinr?.let { "$it dB" } ?: "--", sinrColor(nr.sinr)),
                        SignalRow("RSSI", nr.rssi?.let { "$it dBm" } ?: "--"),
                        SignalRow("Band", if (nr.band.isNotBlank()) "n${nr.band}" else "--"),
                        SignalRow("PCI", nr.pci.ifBlank { "--" }),
                        SignalRow("Cell ID", nr.cellId.ifBlank { "--" }),
                        SignalRow("ARFCN", nr.arfcn.ifBlank { "--" }),
                        SignalRow("Bandwidth", nr.bandwidth.ifBlank { "--" }),
                        SignalRow("CA", nr.ca.ifBlank { "--" }),
                    ),
                )

                // LTE Signal Panel
                val lte = state.current.lte
                SignalPanel(
                    title = "LTE",
                    rows = listOf(
                        SignalRow("RSRP", lte.rsrp?.let { "$it dBm" } ?: "--", rsrpColor(lte.rsrp)),
                        SignalRow("RSRQ", lte.rsrq?.let { "$it dB" } ?: "--"),
                        SignalRow("SINR", lte.sinr?.let { "$it dB" } ?: "--", sinrColor(lte.sinr)),
                        SignalRow("RSSI", lte.rssi?.let { "$it dBm" } ?: "--"),
                        SignalRow("CA", lte.ca.ifBlank { "--" }),
                        SignalRow("CA State", lte.caState.ifBlank { "--" }),
                    ),
                )

                // WCDMA Signal Panel
                val wcdma = state.current.wcdma
                if (wcdma.rscp != null || wcdma.ecio != null) {
                    SignalPanel(
                        title = "WCDMA",
                        rows = listOf(
                            SignalRow("RSCP", wcdma.rscp?.let { "$it dBm" } ?: "--"),
                            SignalRow("Ec/Io", wcdma.ecio?.let { "$it dB" } ?: "--"),
                        ),
                    )
                }

                // RSRP History chart
                if (state.history.size > 1) {
                    RSRPHistoryCard(state.history)
                }
            }
        }
    }
}

private data class SignalRow(
    val label: String,
    val value: String,
    val color: Color = Color.Unspecified,
)

@Composable
private fun SignalPanel(title: String, rows: List<SignalRow>) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
            )
            Spacer(modifier = Modifier.height(8.dp))
            rows.forEach { row ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 2.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        row.label,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        row.value,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        color = if (row.color != Color.Unspecified) row.color else MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        }
    }
}

@Composable
private fun RSRPHistoryCard(history: List<com.ztecompanion.core.model.SignalSnapshot>) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                "RSRP History",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
            )
            Spacer(modifier = Modifier.height(8.dp))

            val nrValues = history.mapNotNull { it.nr.rsrp }
            val lteValues = history.mapNotNull { it.lte.rsrp }

            if (nrValues.isNotEmpty()) {
                Text(
                    "NR: min ${nrValues.min()} / avg ${nrValues.average().toInt()} / max ${nrValues.max()} dBm",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            if (lteValues.isNotEmpty()) {
                Text(
                    "LTE: min ${lteValues.min()} / avg ${lteValues.average().toInt()} / max ${lteValues.max()} dBm",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))

            // Simple text-based sparkline for RSRP history
            val values = nrValues.ifEmpty { lteValues }
            if (values.size >= 2) {
                val barChars = "\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588"
                val minVal = values.min().toFloat()
                val maxVal = values.max().toFloat()
                val range = (maxVal - minVal).coerceAtLeast(1f)
                val sparkline = values.joinToString("") { v ->
                    val idx = ((v - minVal) / range * (barChars.length - 1)).toInt()
                        .coerceIn(0, barChars.length - 1)
                    barChars[idx].toString()
                }
                Text(
                    sparkline,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}

private fun rsrpColor(rsrp: Int?): Color {
    if (rsrp == null) return Color.Unspecified
    return when {
        rsrp >= -80 -> Color(0xFF4CAF50)
        rsrp >= -100 -> Color(0xFFFFEB3B)
        rsrp >= -110 -> Color(0xFFFF9800)
        else -> Color(0xFFF44336)
    }
}

private fun sinrColor(sinr: Int?): Color {
    if (sinr == null) return Color.Unspecified
    return when {
        sinr >= 20 -> Color(0xFF4CAF50)
        sinr >= 10 -> Color(0xFFFFEB3B)
        sinr >= 0 -> Color(0xFFFF9800)
        else -> Color(0xFFF44336)
    }
}

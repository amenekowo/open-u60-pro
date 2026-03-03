package com.ztecompanion.feature.dashboard

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.ztecompanion.core.model.formatSpeed
import com.ztecompanion.core.network.AuthState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DashboardScreen(
    onNavigateToSignal: () -> Unit,
    onNavigateToLogin: () -> Unit,
    viewModel: DashboardViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()
    val authState by viewModel.authState.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("ZTE Companion") },
                actions = {
                    if (authState != AuthState.LOGGED_IN) {
                        TextButton(onClick = onNavigateToLogin) {
                            Text("Login")
                        }
                    }
                },
            )
        },
    ) { padding ->
        if (authState != AuthState.LOGGED_IN) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(
                        Icons.Default.Router,
                        contentDescription = null,
                        modifier = Modifier.size(64.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text("Not connected", style = MaterialTheme.typography.titleMedium)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "Login to view dashboard",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Button(onClick = onNavigateToLogin) {
                        Text("Login")
                    }
                }
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

                // Operator + network type
                val op = state.signal.operator
                if (op.provider.isNotBlank()) {
                    Text(
                        "${op.provider} - ${op.networkType}",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    val rsrp = state.signal.nr.rsrp ?: state.signal.lte.rsrp
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.SignalCellularAlt,
                        title = "Signal",
                        value = if (rsrp != null) "$rsrp dBm" else "--",
                        subtitle = signalQualityLabel(rsrp),
                        valueColor = rsrpColor(rsrp),
                        onClick = onNavigateToSignal,
                    )
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.BatteryStd,
                        title = "Battery",
                        value = "${state.battery.capacity}%",
                        subtitle = "${state.battery.temperature}\u00B0C",
                        valueColor = batteryColor(state.battery.capacity),
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.Download,
                        title = "Download",
                        value = formatSpeed(state.traffic.rxBytesPerSec),
                        subtitle = "",
                    )
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.Upload,
                        title = "Upload",
                        value = formatSpeed(state.traffic.txBytesPerSec),
                        subtitle = "",
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.Devices,
                        title = "Clients",
                        value = "${state.clientCount}",
                        subtitle = "connected",
                    )
                    DashboardCard(
                        modifier = Modifier.weight(1f),
                        icon = Icons.Default.Thermostat,
                        title = "CPU Temp",
                        value = "${state.thermal.cpuTemp}\u00B0C",
                        subtitle = if (state.thermal.cpuTemp > 70) "High" else "Normal",
                        valueColor = if (state.thermal.cpuTemp > 70) Color(0xFFF44336) else Color.Unspecified,
                    )
                }

                // NR band info
                val nrBand = state.signal.nr.band
                if (nrBand.isNotBlank()) {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Column(modifier = Modifier.padding(16.dp)) {
                            Text("NR Band", style = MaterialTheme.typography.labelMedium)
                            Text(
                                "n$nrBand",
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.Bold,
                            )
                            if (state.signal.nr.ca.isNotBlank()) {
                                Text(
                                    "CA: ${state.signal.nr.ca}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DashboardCard(
    modifier: Modifier = Modifier,
    icon: ImageVector,
    title: String,
    value: String,
    subtitle: String,
    valueColor: Color = Color.Unspecified,
    onClick: (() -> Unit)? = null,
) {
    Card(
        modifier = modifier,
        onClick = onClick ?: {},
        enabled = onClick != null,
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    icon,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.width(6.dp))
                Text(
                    title,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                value,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = if (valueColor != Color.Unspecified) valueColor else MaterialTheme.colorScheme.onSurface,
            )
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
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

private fun signalQualityLabel(rsrp: Int?): String {
    if (rsrp == null) return "No signal"
    return when {
        rsrp >= -80 -> "Excellent"
        rsrp >= -100 -> "Good"
        rsrp >= -110 -> "Fair"
        else -> "Poor"
    }
}

private fun batteryColor(capacity: Int): Color {
    return when {
        capacity > 50 -> Color(0xFF4CAF50)
        capacity > 20 -> Color(0xFFFF9800)
        else -> Color(0xFFF44336)
    }
}

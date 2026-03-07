package com.ztecompanion.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.ztecompanion.feature.bandlock.BandLockScreen
import com.ztecompanion.feature.clients.ClientsScreen
import com.ztecompanion.feature.config.ConfigToolScreen
import com.ztecompanion.feature.dashboard.DashboardScreen
import com.ztecompanion.feature.deviceinfo.DeviceInfoScreen
import com.ztecompanion.feature.login.LoginScreen
import com.ztecompanion.feature.router.RouterSettingsListScreen
import com.ztecompanion.feature.router.apn.APNScreen
import com.ztecompanion.feature.router.celllock.CellLockScreen as RouterCellLockScreen
import com.ztecompanion.feature.router.firewall.FirewallSettingsScreen
import com.ztecompanion.feature.router.mobilenetwork.MobileNetworkScreen
import com.ztecompanion.feature.router.networkmode.NetworkModeScreen
import com.ztecompanion.feature.router.device.DeviceControlScreen
import com.ztecompanion.feature.router.lan.LANSettingsScreen
import com.ztecompanion.feature.router.sim.SIMScreen
import com.ztecompanion.feature.router.stc.STCScreen
import com.ztecompanion.feature.router.stk.STKScreen
import com.ztecompanion.feature.router.qos.QoSScreen
import com.ztecompanion.feature.router.telemetry.TelemetryBlockerScreen
import com.ztecompanion.feature.router.vpn.VPNPassthroughScreen
import com.ztecompanion.feature.router.dns.DNSSettingsScreen
import com.ztecompanion.feature.router.dns.DoHCacheInspectorScreen
import com.ztecompanion.feature.router.wifi.GuestWiFiSettingsScreen
import com.ztecompanion.feature.router.wifi.WiFiSettingsScreen
import com.ztecompanion.feature.settings.SettingsScreen
import com.ztecompanion.feature.signal.SignalMonitorScreen
import com.ztecompanion.feature.sms.SMSComposeScreen
import com.ztecompanion.feature.sms.SMSConversationScreen
import com.ztecompanion.feature.sms.SMSListScreen
import com.ztecompanion.feature.router.schedule.ScheduleRebootScreen
import com.ztecompanion.feature.router.signaldetect.SignalDetectScreen
import com.ztecompanion.feature.scheduler.SchedulerFormScreen
import com.ztecompanion.feature.scheduler.SchedulerListScreen
import com.ztecompanion.feature.tools.EnableADBScreen
import com.ztecompanion.feature.tools.PlaceholderScreen
import com.ztecompanion.feature.tools.ToolsListScreen
import com.ztecompanion.feature.tools.speedtest.LANSpeedTestScreen
import com.ztecompanion.feature.tools.speedtest.SpeedTestScreen
import com.ztecompanion.feature.usb.USBModeScreen

sealed class Screen(val route: String) {
    // Bottom tabs
    data object Dashboard : Screen("dashboard")
    data object SMSList : Screen("sms")
    data object Router : Screen("router")
    data object Tools : Screen("tools")
    data object Settings : Screen("settings")

    // SMS
    data object SMSConversation : Screen("sms_conversation/{number}") {
        fun createRoute(number: String) = "sms_conversation/$number"
    }
    data object SMSCompose : Screen("sms_compose")

    // Signal
    data object Signal : Screen("signal")

    // Auth
    data object Login : Screen("login")

    // Tools sub-screens
    data object DeviceInfo : Screen("device_info")
    data object Clients : Screen("clients")
    data object BandLock : Screen("band_lock")
    data object EnableADB : Screen("enable_adb")
    data object ConfigTool : Screen("config_tool")
    data object SpeedTest : Screen("tools/speed_test")
    data object LANSpeedTest : Screen("tools/lan_speed_test")

    // Router settings sub-screens
    data object MobileNetwork : Screen("router/mobile_network")
    data object NetworkMode : Screen("router/network_mode")
    data object CellLock : Screen("router/cell_lock")
    data object SIM : Screen("router/sim")
    data object STK : Screen("router/stk")
    data object WiFiSettings : Screen("router/wifi")
    data object GuestWiFi : Screen("router/guest_wifi")
    data object APN : Screen("router/apn")
    data object LANSettings : Screen("router/lan")
    data object DNSSettings : Screen("router/dns")
    data object Firewall : Screen("router/firewall")
    data object TelemetryBlocker : Screen("router/telemetry_blocker")
    data object VPNPassthrough : Screen("router/vpn_passthrough")
    data object QoS : Screen("router/qos")
    data object DeviceControl : Screen("router/device_control")
    data object Scheduler : Screen("router/scheduler")
    data object SchedulerForm : Screen("router/scheduler_form")
    data object USBMode : Screen("router/usb_mode")
    data object STC : Screen("router/stc")
    data object SignalDetect : Screen("router/signal_detect")
    data object ScheduleReboot : Screen("router/schedule_reboot")
    data object DoHCache : Screen("router/doh_cache")

    data object Placeholder : Screen("placeholder/{title}") {
        fun createRoute(title: String) = "placeholder/$title"
    }
}

data class BottomNavItem(
    val screen: Screen,
    val label: String,
    val icon: ImageVector,
)

val bottomNavItems = listOf(
    BottomNavItem(Screen.Dashboard, "Dashboard", Icons.Default.Dashboard),
    BottomNavItem(Screen.SMSList, "SMS", Icons.Default.Sms),
    BottomNavItem(Screen.Router, "Router", Icons.Default.Router),
    BottomNavItem(Screen.Tools, "Tools", Icons.Default.Build),
    BottomNavItem(Screen.Settings, "Settings", Icons.Default.Settings),
)

@Composable
fun AppNavigation() {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination

    val showBottomBar = bottomNavItems.any { item ->
        currentDestination?.hierarchy?.any { it.route == item.screen.route } == true
    }

    Scaffold(
        bottomBar = {
            if (showBottomBar) {
                NavigationBar {
                    bottomNavItems.forEach { item ->
                        val selected = currentDestination?.hierarchy?.any { it.route == item.screen.route } == true
                        NavigationBarItem(
                            icon = { Icon(item.icon, contentDescription = item.label) },
                            label = { Text(item.label) },
                            selected = selected,
                            onClick = {
                                navController.navigate(item.screen.route) {
                                    popUpTo(navController.graph.findStartDestination().id) {
                                        saveState = true
                                    }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                        )
                    }
                }
            }
        },
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Dashboard.route,
            modifier = Modifier.padding(innerPadding),
        ) {
            // Bottom tabs
            composable(Screen.Dashboard.route) {
                DashboardScreen(
                    onNavigateToSignal = { navController.navigate(Screen.Signal.route) },
                    onNavigateToLogin = { navController.navigate(Screen.Login.route) },
                )
            }
            composable(Screen.SMSList.route) {
                SMSListScreen(
                    onNavigateToConversation = { number ->
                        navController.navigate(Screen.SMSConversation.createRoute(number))
                    },
                    onNavigateToCompose = { navController.navigate(Screen.SMSCompose.route) },
                    onNavigateToLogin = { navController.navigate(Screen.Login.route) },
                )
            }
            composable(Screen.Router.route) {
                RouterSettingsListScreen(
                    onNavigateToMobileNetwork = { navController.navigate(Screen.MobileNetwork.route) },
                    onNavigateToNetworkMode = { navController.navigate(Screen.NetworkMode.route) },
                    onNavigateToCellLock = { navController.navigate(Screen.CellLock.route) },
                    onNavigateToSTC = { navController.navigate(Screen.STC.route) },
                    onNavigateToSignalDetect = { navController.navigate(Screen.SignalDetect.route) },
                    onNavigateToSIM = { navController.navigate(Screen.SIM.route) },
                    onNavigateToSTK = { navController.navigate(Screen.STK.route) },
                    onNavigateToWiFi = { navController.navigate(Screen.WiFiSettings.route) },
                    onNavigateToGuestWiFi = { navController.navigate(Screen.GuestWiFi.route) },
                    onNavigateToAPN = { navController.navigate(Screen.APN.route) },
                    onNavigateToLAN = { navController.navigate(Screen.LANSettings.route) },
                    onNavigateToDNS = { navController.navigate(Screen.DNSSettings.route) },
                    onNavigateToFirewall = { navController.navigate(Screen.Firewall.route) },
                    onNavigateToTelemetryBlocker = { navController.navigate(Screen.TelemetryBlocker.route) },
                    onNavigateToVPNPassthrough = { navController.navigate(Screen.VPNPassthrough.route) },
                    onNavigateToQoS = { navController.navigate(Screen.QoS.route) },
                    onNavigateToDeviceControl = { navController.navigate(Screen.DeviceControl.route) },
                    onNavigateToScheduleReboot = { navController.navigate(Screen.ScheduleReboot.route) },
                )
            }
            composable(Screen.Tools.route) {
                ToolsListScreen(
                    onNavigateToDeviceInfo = { navController.navigate(Screen.DeviceInfo.route) },
                    onNavigateToClients = { navController.navigate(Screen.Clients.route) },
                    onNavigateToBandLock = { navController.navigate(Screen.BandLock.route) },
                    onNavigateToEnableADB = { navController.navigate(Screen.EnableADB.route) },
                    onNavigateToConfigTool = { navController.navigate(Screen.ConfigTool.route) },
                    onNavigateToScheduler = { navController.navigate(Screen.Scheduler.route) },
                    onNavigateToUSBMode = { navController.navigate(Screen.USBMode.route) },
                    onNavigateToSpeedTest = { navController.navigate(Screen.SpeedTest.route) },
                    onNavigateToLANSpeedTest = { navController.navigate(Screen.LANSpeedTest.route) },
                    onNavigateToPlaceholder = { title ->
                        navController.navigate(Screen.Placeholder.createRoute(title))
                    },
                )
            }
            composable(Screen.Settings.route) {
                SettingsScreen(
                    onNavigateToLogin = { navController.navigate(Screen.Login.route) },
                )
            }

            // SMS sub-screens
            composable(Screen.SMSConversation.route) { backStackEntry ->
                val number = backStackEntry.arguments?.getString("number") ?: ""
                SMSConversationScreen(
                    normalizedNumber = number,
                    onBack = { navController.popBackStack() },
                )
            }
            composable(Screen.SMSCompose.route) {
                SMSComposeScreen(
                    onBack = { navController.popBackStack() },
                )
            }

            // Signal
            composable(Screen.Signal.route) {
                SignalMonitorScreen()
            }

            // Auth
            composable(Screen.Login.route) {
                LoginScreen(
                    onLoginSuccess = { navController.popBackStack() },
                    onDismiss = { navController.popBackStack() },
                )
            }

            // Tools sub-screens
            composable(Screen.DeviceInfo.route) {
                DeviceInfoScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.Clients.route) {
                ClientsScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.BandLock.route) {
                BandLockScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.EnableADB.route) {
                EnableADBScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.ConfigTool.route) {
                ConfigToolScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.SpeedTest.route) {
                SpeedTestScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.LANSpeedTest.route) {
                LANSpeedTestScreen(onBack = { navController.popBackStack() })
            }

            // Router settings sub-screens
            composable(Screen.MobileNetwork.route) {
                MobileNetworkScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.NetworkMode.route) {
                NetworkModeScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.CellLock.route) {
                RouterCellLockScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.SIM.route) {
                SIMScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.STK.route) {
                STKScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.WiFiSettings.route) {
                WiFiSettingsScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.GuestWiFi.route) {
                GuestWiFiSettingsScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.APN.route) {
                APNScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.LANSettings.route) {
                LANSettingsScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.DNSSettings.route) {
                DNSSettingsScreen(
                    onBack = { navController.popBackStack() },
                    onNavigateToCache = { navController.navigate(Screen.DoHCache.route) },
                )
            }
            composable(Screen.DoHCache.route) {
                DoHCacheInspectorScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.Firewall.route) {
                FirewallSettingsScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.TelemetryBlocker.route) {
                TelemetryBlockerScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.VPNPassthrough.route) {
                VPNPassthroughScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.QoS.route) {
                QoSScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.DeviceControl.route) {
                DeviceControlScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.Scheduler.route) {
                SchedulerListScreen(
                    onBack = { navController.popBackStack() },
                    onNavigateToForm = { navController.navigate(Screen.SchedulerForm.route) },
                )
            }
            composable(Screen.SchedulerForm.route) {
                SchedulerFormScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.USBMode.route) {
                USBModeScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.STC.route) {
                STCScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.SignalDetect.route) {
                SignalDetectScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.ScheduleReboot.route) {
                ScheduleRebootScreen(onBack = { navController.popBackStack() })
            }
            composable(Screen.Placeholder.route) { backStackEntry ->
                val title = backStackEntry.arguments?.getString("title") ?: "Feature"
                PlaceholderScreen(title = title, onBack = { navController.popBackStack() })
            }
        }
    }
}

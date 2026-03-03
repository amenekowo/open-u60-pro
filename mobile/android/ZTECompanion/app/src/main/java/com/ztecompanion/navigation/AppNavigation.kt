package com.ztecompanion.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.SignalCellularAlt
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
import com.ztecompanion.feature.settings.SettingsScreen
import com.ztecompanion.feature.signal.SignalMonitorScreen
import com.ztecompanion.feature.tools.EnableADBScreen
import com.ztecompanion.feature.tools.PlaceholderScreen
import com.ztecompanion.feature.tools.ToolsListScreen

sealed class Screen(val route: String) {
    data object Dashboard : Screen("dashboard")
    data object Signal : Screen("signal")
    data object Tools : Screen("tools")
    data object Settings : Screen("settings")
    data object Login : Screen("login")
    data object DeviceInfo : Screen("device_info")
    data object Clients : Screen("clients")
    data object BandLock : Screen("band_lock")
    data object EnableADB : Screen("enable_adb")
    data object ConfigTool : Screen("config_tool")
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
    BottomNavItem(Screen.Signal, "Signal", Icons.Default.SignalCellularAlt),
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
            composable(Screen.Dashboard.route) {
                DashboardScreen(
                    onNavigateToSignal = { navController.navigate(Screen.Signal.route) },
                    onNavigateToLogin = { navController.navigate(Screen.Login.route) },
                )
            }
            composable(Screen.Signal.route) {
                SignalMonitorScreen()
            }
            composable(Screen.Tools.route) {
                ToolsListScreen(
                    onNavigateToDeviceInfo = { navController.navigate(Screen.DeviceInfo.route) },
                    onNavigateToClients = { navController.navigate(Screen.Clients.route) },
                    onNavigateToBandLock = { navController.navigate(Screen.BandLock.route) },
                    onNavigateToEnableADB = { navController.navigate(Screen.EnableADB.route) },
                    onNavigateToConfigTool = { navController.navigate(Screen.ConfigTool.route) },
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
            composable(Screen.Login.route) {
                LoginScreen(
                    onLoginSuccess = { navController.popBackStack() },
                    onDismiss = { navController.popBackStack() },
                )
            }
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
            composable(Screen.Placeholder.route) { backStackEntry ->
                val title = backStackEntry.arguments?.getString("title") ?: "Feature"
                PlaceholderScreen(title = title, onBack = { navController.popBackStack() })
            }
        }
    }
}

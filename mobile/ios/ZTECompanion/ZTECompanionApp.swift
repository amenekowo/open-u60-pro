import SwiftUI

@main
struct ZTECompanionApp: App {
    @State private var authManager: AuthManager
    @AppStorage("gateway_ip") private var gatewayIP: String = "192.168.0.1"
    @AppStorage("dark_mode_override") private var darkModeOverride: Int = 0

    private let client: UbusClient

    init() {
        let savedIP = UserDefaults.standard.string(forKey: "gateway_ip") ?? "192.168.0.1"
        let ubusClient = UbusClient(gatewayIP: savedIP)
        self.client = ubusClient
        _authManager = State(initialValue: AuthManager(client: ubusClient))
    }

    var body: some Scene {
        WindowGroup {
            rootView
                .preferredColorScheme(colorScheme)
                .onChange(of: gatewayIP) {
                    client.gatewayIP = gatewayIP
                }
        }
    }

    @ViewBuilder
    private var rootView: some View {
        if authManager.isAuthenticated {
            TabBarView(client: client, authManager: authManager)
        } else {
            LoginView(authManager: authManager)
                .task {
                    // Try auto-login from Keychain
                    _ = await authManager.reauthenticate()
                }
        }
    }

    private var colorScheme: ColorScheme? {
        switch darkModeOverride {
        case 1: return .light
        case 2: return .dark
        default: return nil
        }
    }
}

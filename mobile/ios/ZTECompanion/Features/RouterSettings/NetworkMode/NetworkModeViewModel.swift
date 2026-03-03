import SwiftUI

@Observable
@MainActor
final class NetworkModeViewModel {
    var config: NetworkModeConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    var selectedNetSelect: String = NetworkModeConfig.netSelectOptions[0].value

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func refresh() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken

        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_netinfo",
                params: [:]
            )
            config = NetworkModeParser.parse(data)
            selectedNetSelect = config.netSelect
        } catch {
            showMessage("Failed to load network mode: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func applyMode() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            if selectedNetSelect != config.netSelect {
                let (_, _) = try await client.call(
                    sessionToken: token,
                    object: "zte_nwinfo_api",
                    method: "nwinfo_set_netselect",
                    params: ["net_select": selectedNetSelect]
                )
            }
            // Poll until the router confirms the new value (up to ~10s)
            let expectedNet = selectedNetSelect
            for _ in 0..<5 {
                try? await Task.sleep(for: .seconds(2))
                let (_, data) = try await client.call(
                    sessionToken: token,
                    object: "zte_nwinfo_api",
                    method: "nwinfo_get_netinfo",
                    params: [:]
                )
                let fetched = NetworkModeParser.parse(data)
                if fetched.netSelect == expectedNet {
                    config = fetched
                    showMessage("Network mode updated", isError: false)
                    isLoading = false
                    return
                }
            }

            // Timeout — keep optimistic update, warn the user
            config = NetworkModeConfig(netSelect: expectedNet)
            showMessage("Mode sent — router may still be switching", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

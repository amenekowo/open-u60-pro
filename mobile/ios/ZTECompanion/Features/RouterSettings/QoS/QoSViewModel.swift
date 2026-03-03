import SwiftUI

@Observable
@MainActor
final class QoSViewModel {
    var config: QoSConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

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
                object: "zwrt_router.api",
                method: "router_get_qos_switch",
                params: [:]
            )
            config = QoSParser.parse(data)
        } catch {
            showMessage("Failed to load QoS: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func toggle(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_qos_switch",
                params: ["qos_switch": enabled ? "1" : "0"]
            )
            showMessage("QoS \(enabled ? "enabled" : "disabled")", isError: false)
            config = QoSConfig(enabled: enabled)
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

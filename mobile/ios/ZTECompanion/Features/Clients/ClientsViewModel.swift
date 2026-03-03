import SwiftUI

@Observable
@MainActor
final class ClientsViewModel {
    var devices: [ConnectedDevice] = []
    var isLoading: Bool = false
    var error: String?

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func refresh() async {
        isLoading = true
        error = nil
        let token = authManager.sessionToken

        do {
            // Fetch host hints
            let (_, hintsData) = try await client.call(
                sessionToken: token, object: "luci-rpc",
                method: "getHostHints", params: [:]
            )
            var deviceList = DeviceParser.parseHostHints(hintsData)

            // Enrich with DHCP lease info
            do {
                let (_, dhcpData) = try await client.call(
                    sessionToken: token, object: "luci-rpc",
                    method: "getDHCPLeases", params: ["family": 4]
                )
                if let leases = dhcpData["dhcp_leases"] as? [[String: Any]] {
                    DeviceParser.enrichWithDHCP(devices: &deviceList, leases: leases)
                }
            } catch {
                // DHCP enrichment is optional
            }

            devices = deviceList
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }
}

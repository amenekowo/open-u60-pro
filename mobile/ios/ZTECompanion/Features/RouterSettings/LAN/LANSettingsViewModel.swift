import SwiftUI

@Observable
@MainActor
final class LANSettingsViewModel {
    var config: LANConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    // Editable fields
    var editLanIP: String = ""
    var editNetmask: String = ""
    var editDhcpEnabled: Bool = false
    var editDhcpStart: String = ""
    var editDhcpEnd: String = ""
    var editLeaseTime: String = ""

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
                method: "router_get_lan_para",
                params: [:]
            )
            config = LANParser.parse(data)
            syncEditFields()
        } catch {
            showMessage("Failed to load LAN: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func apply() async {
        guard !editLanIP.isEmpty else {
            showMessage("LAN IP is required", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_lan_para",
                params: [
                    "lan_ipaddr": editLanIP,
                    "lan_netmask": editNetmask,
                    "dhcp_enable": editDhcpEnabled ? "1" : "0",
                    "dhcp_start": editDhcpStart,
                    "dhcp_end": editDhcpEnd,
                    "dhcp_lease_time": editLeaseTime
                ]
            )
            showMessage("LAN settings updated", isError: false)
            config = LANConfig(lanIP: editLanIP, netmask: editNetmask, dhcpEnabled: editDhcpEnabled,
                               dhcpStart: editDhcpStart, dhcpEnd: editDhcpEnd, dhcpLeaseTime: editLeaseTime)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    private func syncEditFields() {
        editLanIP = config.lanIP
        editNetmask = config.netmask
        editDhcpEnabled = config.dhcpEnabled
        editDhcpStart = config.dhcpStart
        editDhcpEnd = config.dhcpEnd
        editLeaseTime = config.dhcpLeaseTime
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

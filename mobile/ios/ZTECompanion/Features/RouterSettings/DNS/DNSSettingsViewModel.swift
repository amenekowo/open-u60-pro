import SwiftUI

@Observable
@MainActor
final class DNSSettingsViewModel {
    var config: DNSConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    // Editable fields
    var editPrimary: String = ""
    var editSecondary: String = ""
    var editMode: Bool = false  // manual = true

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
                method: "router_get_dns_para",
                params: [:]
            )
            config = DNSParser.parse(data)
            editPrimary = config.primaryDns
            editSecondary = config.secondaryDns
            editMode = config.isManual
        } catch {
            showMessage("Failed to load DNS: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func applyDNS() async {
        guard editMode else {
            // Switch to auto
            await setDNS(mode: "auto", primary: "", secondary: "")
            return
        }

        guard !editPrimary.isEmpty else {
            showMessage("Primary DNS cannot be empty", isError: true)
            return
        }

        await setDNS(mode: "manual", primary: editPrimary, secondary: editSecondary)
    }

    private func setDNS(mode: String, primary: String, secondary: String) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_wan_dns",
                params: [
                    "wan_dns_mode": mode,
                    "wan_prefer_dns_manual": primary,
                    "wan_standby_dns_manual": secondary
                ]
            )
            showMessage("DNS updated to \(mode)", isError: false)
            config = DNSConfig(wanDnsMode: mode, primaryDns: primary, secondaryDns: secondary,
                               ipv6PrimaryDns: config.ipv6PrimaryDns, ipv6SecondaryDns: config.ipv6SecondaryDns,
                               ipv6DnsMode: config.ipv6DnsMode)
            editPrimary = primary
            editSecondary = secondary
            editMode = mode == "manual"
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

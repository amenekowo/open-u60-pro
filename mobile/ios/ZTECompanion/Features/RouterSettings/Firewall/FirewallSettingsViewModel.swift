import SwiftUI

@Observable
@MainActor
final class FirewallSettingsViewModel {
    var config: FirewallConfig = .empty
    var portForwardRules: [PortForwardRule] = []
    var filterRules: [FilterRule] = []
    var upnpEnabled: Bool = false
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var showAddPortForward: Bool = false

    // DMZ edit fields
    var editDmzEnabled: Bool = false
    var editDmzIP: String = ""

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
                method: "router_get_firewall_para",
                params: [:]
            )
            config = FirewallParser.parseConfig(data)
            editDmzEnabled = config.dmzEnabled
            editDmzIP = config.dmzHost

            async let pfResult = fetchPortForwardRules(token: token)
            async let filterResult = fetchFilterRules(token: token)
            async let upnpResult = fetchUPnP(token: token)

            portForwardRules = await pfResult
            filterRules = await filterResult
            upnpEnabled = await upnpResult
        } catch {
            showMessage("Failed to load firewall: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    private func fetchPortForwardRules(token: String) async -> [PortForwardRule] {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_get_portforward_rule",
                params: [:]
            )
            return FirewallParser.parsePortForwardRules(data)
        } catch {
            return []
        }
    }

    private func fetchFilterRules(token: String) async -> [FilterRule] {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_get_macipport_filter_rule",
                params: [:]
            )
            return FirewallParser.parseFilterRules(data)
        } catch {
            return []
        }
    }

    private func fetchUPnP(token: String) async -> Bool {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_get_upnp_switch",
                params: [:]
            )
            if let str = data["upnp_switch"] as? String {
                return str == "1"
            }
            return false
        } catch {
            return false
        }
    }

    func toggleFirewall(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_firewall_switch",
                params: ["firewall_switch": enabled ? "1" : "0"]
            )
            showMessage("Firewall \(enabled ? "enabled" : "disabled")", isError: false)
            config.enabled = enabled
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func setLevel(_ level: String) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_firewall_level",
                params: ["firewall_level": level]
            )
            showMessage("Firewall level set to \(level)", isError: false)
            config.level = level
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func toggleNAT(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_nat_switch",
                params: ["nat_switch": enabled ? "1" : "0"]
            )
            showMessage("NAT \(enabled ? "enabled" : "disabled")", isError: false)
            config.nat = enabled
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func toggleUPnP(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_upnp_switch",
                params: ["upnp_switch": enabled ? "1" : "0"]
            )
            showMessage("UPnP \(enabled ? "enabled" : "disabled")", isError: false)
            upnpEnabled = enabled
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func applyDMZ() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_dmz",
                params: [
                    "dmz_enabled": editDmzEnabled ? "1" : "0",
                    "dmz_ip": editDmzIP
                ]
            )
            showMessage("DMZ settings updated", isError: false)
            config.dmzEnabled = editDmzEnabled
            config.dmzHost = editDmzIP
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func togglePortForward(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_portforward_switch",
                params: ["port_forward_switch": enabled ? "1" : "0"]
            )
            showMessage("Port forwarding \(enabled ? "enabled" : "disabled")", isError: false)
            config.portForwardEnabled = enabled
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func addPortForward(name: String, protocol_: String, wanPort: String, lanIP: String, lanPort: String) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_portforward",
                params: [
                    "action": "add",
                    "name": name,
                    "protocol": protocol_,
                    "wan_port": wanPort,
                    "lan_ip": lanIP,
                    "lan_port": lanPort,
                    "enabled": "1"
                ]
            )
            showAddPortForward = false
            showMessage("Port forward rule added", isError: false)
            portForwardRules.append(PortForwardRule(id: UUID().uuidString, name: name, protocol_: protocol_, wanPort: wanPort, lanIP: lanIP, lanPort: lanPort, enabled: true))
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func deletePortForward(_ rule: PortForwardRule) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_router.api",
                method: "router_set_portforward",
                params: [
                    "action": "delete",
                    "id": rule.id
                ]
            )
            showMessage("Port forward rule deleted", isError: false)
            portForwardRules.removeAll { $0.id == rule.id }
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

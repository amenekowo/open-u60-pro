import SwiftUI

@Observable
@MainActor
final class TelemetryBlockerViewModel {
    var filterConfig: DomainFilterConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var newDomain: String = ""

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
                object: "zwrt_smart_mng.api",
                method: "smart_mng_domain_filter_get",
                params: [:]
            )
            filterConfig = TelemetryParser.parseDomainFilter(data)
        } catch {
            showMessage("Failed to load filters: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func toggleFilter(enabled: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_smart_mng.api",
                method: "smart_mng_domain_filter_set",
                params: ["enable": enabled ? "1" : "0"]
            )
            showMessage("Domain filter \(enabled ? "enabled" : "disabled")", isError: false)
            filterConfig.enabled = enabled
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func addDomain(_ domain: String) async {
        let trimmed = domain.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            showMessage("Enter a domain name", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_smart_mng.api",
                method: "smart_mng_domain_filter_set",
                params: [
                    "action": "add",
                    "domain": trimmed,
                    "enabled": "1"
                ]
            )
            newDomain = ""
            showMessage("Added \(trimmed)", isError: false)
            filterConfig.rules.append(DomainFilterRule(id: UUID().uuidString, domain: trimmed, enabled: true))
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func removeDomain(_ rule: DomainFilterRule) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_smart_mng.api",
                method: "smart_mng_domain_filter_set",
                params: [
                    "action": "delete",
                    "id": rule.id
                ]
            )
            showMessage("Removed \(rule.domain)", isError: false)
            filterConfig.rules.removeAll { $0.id == rule.id }
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func blockAllTelemetry() async {
        isLoading = true
        let token = authManager.sessionToken
        let existingDomains = Set(filterConfig.rules.map(\.domain))

        var added = 0
        for domain in TelemetryParser.knownTelemetryDomains {
            guard !existingDomains.contains(domain) else { continue }
            do {
                let (_, _) = try await client.call(
                    sessionToken: token,
                    object: "zwrt_smart_mng.api",
                    method: "smart_mng_domain_filter_set",
                    params: [
                        "action": "add",
                        "domain": domain,
                        "enabled": "1"
                    ]
                )
                added += 1
                filterConfig.rules.append(DomainFilterRule(id: UUID().uuidString, domain: domain, enabled: true))
            } catch {
                // Continue with remaining domains
            }
        }

        if added > 0 {
            showMessage("Blocked \(added) telemetry domain\(added == 1 ? "" : "s")", isError: false)
        } else {
            showMessage("All telemetry domains already blocked", isError: false)
        }

        isLoading = false
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

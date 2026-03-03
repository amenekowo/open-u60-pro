import SwiftUI

@Observable
@MainActor
final class STCViewModel {
    var config: STCConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    var editLteTimer: String = ""
    var editNrsaTimer: String = ""
    var editLteMax: String = ""
    var editNrsaMax: String = ""

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
            let (_, paramsData) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_stc_white_list_par",
                params: [:]
            )
            config = STCParser.parseParams(paramsData)

            if let (_, statusData) = try? await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_stc_white_list_status",
                params: [:]
            ) {
                config = STCParser.parseStatus(statusData, into: config)
            }

            editLteTimer = config.lteCollectTimer
            editNrsaTimer = config.nrsaCollectTimer
            editLteMax = config.lteWhitelistMax
            editNrsaMax = config.nrsaWhitelistMax
        } catch {
            showMessage("Failed to load STC: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func applyParams() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_set_stc_white_list_par",
                params: [
                    "lte_collect_timer": editLteTimer,
                    "nrsa_collect_timer": editNrsaTimer,
                    "lte_whitelist_max": editLteMax,
                    "nrsa_whitelist_max": editNrsaMax
                ]
            )
            showMessage("STC parameters updated", isError: false)
            config.lteCollectTimer = editLteTimer
            config.nrsaCollectTimer = editNrsaTimer
            config.lteWhitelistMax = editLteMax
            config.nrsaWhitelistMax = editNrsaMax
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func enable() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_stc_cell_lock_enable",
                params: [:]
            )
            showMessage("STC enabled", isError: false)
            config.enabled = true
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func disable() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_stc_cell_lock_disable",
                params: [:]
            )
            showMessage("STC disabled", isError: false)
            config.enabled = false
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func reset() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_stc_cell_lock_reset",
                params: [:]
            )
            showMessage("STC whitelist reset", isError: false)
            config.enabled = false
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

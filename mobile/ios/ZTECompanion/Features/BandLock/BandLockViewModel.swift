import SwiftUI

@Observable
@MainActor
final class BandLockViewModel {
    var config: BandConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func toggleNRBand(_ band: String) {
        if config.nrBands.contains(band) {
            config.nrBands.remove(band)
        } else {
            config.nrBands.insert(band)
        }
    }

    func toggleLTEBand(_ band: String) {
        if config.lteBands.contains(band) {
            config.lteBands.remove(band)
        } else {
            config.lteBands.insert(band)
        }
    }

    func applyNRLock() async {
        guard !config.nrBands.isEmpty else {
            showMessage("Select at least one NR band", isError: true)
            return
        }
        isLoading = true
        let token = authManager.sessionToken
        let bandStr = config.nrBandString

        do {
            // Lock NSA bands
            let (_, _) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_set_nrbandlock",
                params: ["nr5g_type": "nsa", "nr5g_band": bandStr]
            )
            // Lock SA bands
            let (_, _) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_set_nrbandlock",
                params: ["nr5g_type": "sa", "nr5g_band": bandStr]
            )
            showMessage("NR bands locked: \(bandStr)", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }
        isLoading = false
    }

    func applyLTELock() async {
        guard !config.lteBands.isEmpty else {
            showMessage("Select at least one LTE band", isError: true)
            return
        }
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_set_gwl_bandlock",
                params: [
                    "is_lte_band": "1",
                    "lte_band_mask": config.lteBandString,
                    "is_gw_band": "0",
                    "gw_band_mask": ""
                ]
            )
            showMessage("LTE bands locked: \(config.lteBandString)", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }
        isLoading = false
    }

    func unlockAll() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_rest_band_rat", params: [:]
            )
            config = .empty
            showMessage("All bands unlocked", isError: false)
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

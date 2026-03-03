import SwiftUI

@Observable
@MainActor
final class MobileNetworkViewModel {
    var config: MobileNetworkConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var isScanning: Bool = false

    var selectedConnectMode: Int = 1
    var selectedRoaming: Bool = false
    var selectedNetSelectMode: String = "auto_select"

    let client: UbusClient
    let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    var hasChanges: Bool {
        selectedConnectMode != config.connectMode
            || selectedRoaming != config.isRoamingEnabled
            || selectedNetSelectMode != config.netSelectMode
    }

    // MARK: - Refresh

    func refresh() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken

        do {
            async let wwanCall = client.call(
                sessionToken: token,
                object: "zwrt_data",
                method: "get_wwaniface",
                params: ["cid": 1]
            )
            async let netInfoCall = client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_netinfo",
                params: [:]
            )
            async let apnModeCall = client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_apn_mode",
                params: [:]
            )
            async let apnListCall = client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_manu_apn_list",
                params: [:]
            )

            let (_, wwanData) = try await wwanCall
            let (_, netInfoData) = try await netInfoCall
            let (_, apnModeData) = try await apnModeCall
            let (_, apnListData) = try await apnListCall

            let wwan = MobileNetworkParser.parseWWAN(wwanData)
            let netSelectMode = MobileNetworkParser.parseNetInfo(netInfoData)
            let currentAPN = MobileNetworkParser.parseCurrentAPN(apnModeData, apnListData)

            config = MobileNetworkConfig(
                connectMode: wwan.connectMode,
                roamEnable: wwan.roamEnable,
                netSelectMode: netSelectMode,
                currentAPN: currentAPN,
                operators: config.operators,
                scanStatus: config.scanStatus
            )
            selectedConnectMode = config.connectMode
            selectedRoaming = config.isRoamingEnabled
            selectedNetSelectMode = config.netSelectMode
        } catch {
            showMessage("Failed to load: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    // MARK: - Apply Connection Settings

    func applySettings() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            // Apply connection mode and roaming via set_wwaniface
            if selectedConnectMode != config.connectMode || selectedRoaming != config.isRoamingEnabled {
                let (_, _) = try await client.call(
                    sessionToken: token,
                    object: "zwrt_data",
                    method: "set_wwaniface",
                    params: [
                        "cid": 1,
                        "connect_mode": selectedConnectMode,
                        "roam_enable": selectedRoaming ? 1 : 0
                    ]
                )
            }

            // Apply network selection mode if changed
            if selectedNetSelectMode != config.netSelectMode {
                if selectedNetSelectMode == "auto_select" {
                    let (_, _) = try await client.call(
                        sessionToken: token,
                        object: "zte_nwinfo_api",
                        method: "nwinfo_set_netselect",
                        params: ["net_select_mode": "auto_select"]
                    )
                }
                // Manual select is handled via scan + register flow
            }

            // Poll to confirm
            for _ in 0..<5 {
                try? await Task.sleep(for: .seconds(2))
                let (_, wwanData) = try await client.call(
                    sessionToken: token,
                    object: "zwrt_data",
                    method: "get_wwaniface",
                    params: ["cid": 1]
                )
                let wwan = MobileNetworkParser.parseWWAN(wwanData)
                if wwan.connectMode == selectedConnectMode && (wwan.roamEnable != 0) == selectedRoaming {
                    config.connectMode = wwan.connectMode
                    config.roamEnable = wwan.roamEnable
                    config.netSelectMode = selectedNetSelectMode
                    showMessage("Settings applied", isError: false)
                    isLoading = false
                    return
                }
            }

            // Optimistic update after timeout
            config.connectMode = selectedConnectMode
            config.roamEnable = selectedRoaming ? 1 : 0
            config.netSelectMode = selectedNetSelectMode
            showMessage("Settings sent — router may still be applying", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    // MARK: - Network Scan

    func scanNetworks() async {
        isScanning = true
        config.operators = []
        config.scanStatus = "scanning"
        message = nil
        let token = authManager.sessionToken

        do {
            // Trigger scan
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_manual_scan",
                params: [:]
            )

            // Poll scan status (up to ~60s — scans take a while)
            for _ in 0..<30 {
                try? await Task.sleep(for: .seconds(2))
                let (_, statusData) = try await client.call(
                    sessionToken: token,
                    object: "zte_nwinfo_api",
                    method: "nwinfo_m_netselect_status",
                    params: [:]
                )
                let status = MobileNetworkParser.parseScanStatus(statusData)
                if status == "done" || status == "complete" || status == "2" {
                    // Fetch results
                    let (_, resultsData) = try await client.call(
                        sessionToken: token,
                        object: "zte_nwinfo_api",
                        method: "nwinfo_m_netselect_contents",
                        params: [:]
                    )
                    config.operators = MobileNetworkParser.parseScanResults(resultsData)
                    config.scanStatus = "done"
                    isScanning = false
                    return
                }
            }

            config.scanStatus = ""
            showMessage("Scan timed out", isError: true)
        } catch {
            config.scanStatus = ""
            showMessage("Scan failed: \(error.localizedDescription)", isError: true)
        }

        isScanning = false
    }

    // MARK: - Manual Register

    func registerNetwork(mccMnc: String, rat: String) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_manual_register",
                params: ["m_mcc_mnc": mccMnc, "m_rat": rat]
            )

            // Poll registration result
            for _ in 0..<15 {
                try? await Task.sleep(for: .seconds(2))
                let (_, resultData) = try await client.call(
                    sessionToken: token,
                    object: "zte_nwinfo_api",
                    method: "nwinfo_m_netselect_result",
                    params: [:]
                )
                let result = MobileNetworkParser.parseRegisterResult(resultData)
                if result == "success" || result == "1" {
                    config.netSelectMode = "manual_select"
                    selectedNetSelectMode = "manual_select"
                    showMessage("Registered to network", isError: false)
                    isLoading = false
                    return
                } else if result == "fail" || result == "0" {
                    showMessage("Registration failed", isError: true)
                    isLoading = false
                    return
                }
            }

            showMessage("Registration timed out", isError: true)
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

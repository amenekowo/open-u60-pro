import SwiftUI

@Observable
@MainActor
final class WiFiSettingsViewModel {
    var config: WiFiConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    // Editable fields
    var editSSID2g: String = ""
    var editSSID5g: String = ""
    var editKey2g: String = ""
    var editKey5g: String = ""
    var editChannel2g: String = "auto"
    var editChannel5g: String = "auto"
    var editTxpower2g: String = "100"
    var editTxpower5g: String = "100"
    var editEncryption2g: String = "psk2+ccmp"
    var editEncryption5g: String = "psk2+ccmp"
    var editHidden2g: Bool = false
    var editHidden5g: Bool = false
    var editWifiOnOff: Bool = true
    var editRadio2gDisabled: Bool = false
    var editRadio5gDisabled: Bool = false
    var editWifi7Enabled: Bool = false
    var editBandwidth2g: String = "auto"
    var editBandwidth5g: String = "auto"

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

        // Try companion wifi_status first (single call, all fields)
        if let (_, companionData) = try? await client.call(
            sessionToken: token,
            object: "zte-companion",
            method: "wifi_status",
            params: [:]
        ), companionData["error"] == nil, companionData["htmode_2g"] != nil {
            config = WiFiParser.parse(companionData)
            syncEditFields()
            isLoading = false
            return
        }

        // Fallback to zwrt_wlan multi-call approach
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "status",
                params: [:]
            )
            config = WiFiParser.parse(data)

            async let zteMbbResult = try? client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "wlan_uci_get_section",
                params: ["section": "zte_mbb"]
            )
            async let wifi0Result = try? client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "wlan_uci_get_section",
                params: ["section": "wifi0"]
            )
            async let wifi1Result = try? client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "wlan_uci_get_section",
                params: ["section": "wifi1"]
            )

            if let (_, zteMbbData) = await zteMbbResult {
                config.wifi7Enabled = WiFiParser.parseWifi7(zteMbbData)
            }
            if let (_, wifi0Data) = await wifi0Result {
                config.bandwidth2g = WiFiParser.parseBandwidth(wifi0Data)
            }
            if let (_, wifi1Data) = await wifi1Result {
                config.bandwidth5g = WiFiParser.parseBandwidth(wifi1Data)
            }

            syncEditFields()
        } catch {
            showMessage("Failed to load WiFi: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func apply() async {
        isLoading = true
        let token = authManager.sessionToken
        let params: [String: Any] = [
            "ssid_2g": editSSID2g,
            "ssid_5g": editSSID5g,
            "key_2g": editKey2g,
            "key_5g": editKey5g,
            "channel_2g": editChannel2g,
            "channel_5g": editChannel5g,
            "txpower_2g": editTxpower2g,
            "txpower_5g": editTxpower5g,
            "encryption_2g": editEncryption2g,
            "encryption_5g": editEncryption5g,
            "hidden_2g": editHidden2g ? "1" : "0",
            "hidden_5g": editHidden5g ? "1" : "0",
            "wifi_onoff": editWifiOnOff ? "1" : "0",
            "radio2_disabled": editRadio2gDisabled ? "1" : "0",
            "radio5_disabled": editRadio5gDisabled ? "1" : "0",
            "wifi6_switch": editWifi7Enabled ? "1" : "0",
            "htmode_2g": editBandwidth2g,
            "htmode_5g": editBandwidth5g
        ]

        do {
            // Try companion wifi_set first
            if let (_, result) = try? await client.call(
                sessionToken: token,
                object: "zte-companion",
                method: "wifi_set",
                params: params
            ), (result["status"] as? String) == "ok" {
                showMessage("WiFi settings applied — WiFi will restart briefly", isError: false)
                updateConfigFromEdits()
                isLoading = false
                return
            }
            // Fallback to zwrt_wlan set
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "set",
                params: params
            )
            showMessage("WiFi settings applied — WiFi will restart briefly", isError: false)
            updateConfigFromEdits()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    private func updateConfigFromEdits() {
        config = WiFiConfig(ssid2g: editSSID2g, ssid5g: editSSID5g, key2g: editKey2g, key5g: editKey5g,
                            channel2g: editChannel2g, channel5g: editChannel5g,
                            txpower2g: editTxpower2g, txpower5g: editTxpower5g,
                            encryption2g: editEncryption2g, encryption5g: editEncryption5g,
                            wifiOnOff: editWifiOnOff, hidden2g: editHidden2g, hidden5g: editHidden5g,
                            radio2gDisabled: editRadio2gDisabled, radio5gDisabled: editRadio5gDisabled,
                            wifi7Enabled: editWifi7Enabled, bandwidth2g: editBandwidth2g, bandwidth5g: editBandwidth5g)
    }

    private func syncEditFields() {
        editSSID2g = config.ssid2g
        editSSID5g = config.ssid5g
        editKey2g = config.key2g
        editKey5g = config.key5g
        editChannel2g = config.channel2g
        editChannel5g = config.channel5g
        editTxpower2g = config.txpower2g
        editTxpower5g = config.txpower5g
        editEncryption2g = config.encryption2g
        editEncryption5g = config.encryption5g
        editHidden2g = config.hidden2g
        editHidden5g = config.hidden5g
        editWifiOnOff = config.wifiOnOff
        editRadio2gDisabled = config.radio2gDisabled
        editRadio5gDisabled = config.radio5gDisabled
        editWifi7Enabled = config.wifi7Enabled
        editBandwidth2g = config.bandwidth2g
        editBandwidth5g = config.bandwidth5g
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

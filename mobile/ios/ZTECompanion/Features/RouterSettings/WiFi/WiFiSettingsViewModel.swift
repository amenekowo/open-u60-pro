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
                object: "zwrt_wlan",
                method: "status",
                params: [:]
            )
            config = WiFiParser.parse(data)
            syncEditFields()
        } catch {
            showMessage("Failed to load WiFi: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func apply() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_wlan",
                method: "set",
                params: [
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
                    "hidden_5g": editHidden5g ? "1" : "0"
                ]
            )
            showMessage("WiFi settings updated", isError: false)
            config = WiFiConfig(ssid2g: editSSID2g, ssid5g: editSSID5g, key2g: editKey2g, key5g: editKey5g,
                                channel2g: editChannel2g, channel5g: editChannel5g,
                                txpower2g: editTxpower2g, txpower5g: editTxpower5g,
                                encryption2g: editEncryption2g, encryption5g: editEncryption5g,
                                wifiOnOff: config.wifiOnOff, hidden2g: editHidden2g, hidden5g: editHidden5g)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
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
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

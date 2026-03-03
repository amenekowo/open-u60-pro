import SwiftUI
import os

private let logger = Logger(subsystem: "com.zte.companion", category: "USBConnection")

@Observable
@MainActor
final class USBConnectionViewModel {
    var usbStatus: USBStatus = .empty
    var showModeSheet: Bool = false
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    private let client: UbusClient
    private let authManager: AuthManager
    private var pollTask: Task<Void, Never>?
    private var wasCableAttached: Bool = false

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func startPolling(interval: TimeInterval = 3.0) {
        stopPolling()
        pollTask = Task {
            while !Task.isCancelled {
                await refresh()
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }

    func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }

    func refresh() async {
        var token = authManager.sessionToken

        var usbData = await fetchUSB(token: token)

        if usbData == nil, await authManager.reauthenticate() {
            token = authManager.sessionToken
            usbData = await fetchUSB(token: token)
        }

        guard let usb = usbData else { return }

        let charger = await fetchCharger(token: token)
        let status = DeviceParser.parseUSBStatus(usb, chargerData: charger)

        if status.cableAttached && !wasCableAttached {
            showModeSheet = true
        }
        wasCableAttached = status.cableAttached

        if status != usbStatus { usbStatus = status }
    }

    func enablePowerbank() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken
        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.powerbank",
                method: "set",
                params: ["state": 1]
            )
            usbStatus.powerbankActive = true
            message = "Fast charging enabled"
            messageIsError = false
        } catch {
            message = "Failed: \(error.localizedDescription)"
            messageIsError = true
        }
        isLoading = false
    }

    func disablePowerbank() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken
        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.powerbank",
                method: "set",
                params: ["state": 0]
            )
            usbStatus.powerbankActive = false
            message = "Fast charging disabled"
            messageIsError = false
        } catch {
            message = "Failed: \(error.localizedDescription)"
            messageIsError = true
        }
        isLoading = false
    }

    private func fetchUSB(token: String) async -> [String: Any]? {
        guard let (_, data) = try? await client.call(
            sessionToken: token,
            object: "zwrt_bsp.usb",
            method: "list",
            params: [:]
        ) else { return nil }
        return data
    }

    private func fetchCharger(token: String) async -> [String: Any]? {
        guard let (_, data) = try? await client.call(
            sessionToken: token,
            object: "zwrt_bsp.charger",
            method: "list",
            params: [:]
        ) else { return nil }
        return data
    }
}

import SwiftUI

@Observable
@MainActor
final class DeviceControlViewModel {
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var showRebootConfirm: Bool = false
    var showFactoryResetConfirm: Bool = false
    var powerSupplyEnabled: Bool = false
    var powerSaveEnabled: Bool = false
    var fastBootEnabled: Bool = false
    private var powerSupplyLoaded: Bool = false
    private var powerSaveLoaded: Bool = false
    private var fastBootLoaded: Bool = false
    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func refresh() async {
        let token = authManager.sessionToken

        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.charger",
                method: "list",
                params: [:]
            )
            let mode = (data["direct_power_supply_mode"] as? String ?? "").lowercased()
            if !mode.isEmpty {
                let newPowerSupply = (mode == "enable" || mode == "1")
                if newPowerSupply != powerSupplyEnabled { powerSupplyEnabled = newPowerSupply }
            }
            powerSupplyLoaded = true
        } catch {
            showMessage("Failed to load power supply status", isError: true)
        }

        do {
            let (_, psData) = try await client.call(
                sessionToken: token,
                object: "zwrt_mc.device.manager",
                method: "get_device_info",
                params: ["deviceInfoList": ["power_saver_mode", "quicken_power_on"]]
            )
            let psMode = psData["power_saver_mode"] as? String ?? ""
            if !psMode.isEmpty {
                let newPowerSave = (psMode == "1")
                if newPowerSave != powerSaveEnabled { powerSaveEnabled = newPowerSave }
            }
            powerSaveLoaded = true

            let fbMode = psData["quicken_power_on"] as? String ?? ""
            if !fbMode.isEmpty {
                let newFastBoot = (fbMode == "1")
                if newFastBoot != fastBootEnabled { fastBootEnabled = newFastBoot }
            }
            fastBootLoaded = true
        } catch {
            showMessage("Failed to load device manager settings", isError: true)
        }
    }

    func setPowerSupply(enabled: Bool) async {
        guard powerSupplyLoaded else { return }
        isLoading = true
        let token = authManager.sessionToken
        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.charger",
                method: "set",
                params: ["direct_power_supply_mode": enabled ? "enable" : "disable"]
            )
            showMessage(enabled ? "Power supply mode enabled" : "Power supply mode disabled", isError: false)
        } catch {
            powerSupplyEnabled = !enabled
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }
        isLoading = false
    }

    func setPowerSave(enabled: Bool) async {
        guard powerSaveLoaded else { return }
        isLoading = true
        let token = authManager.sessionToken
        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_mc.device.manager",
                method: "set_device_info",
                params: ["deviceInfoList": ["power_saver_mode": enabled ? "1" : "0"]]
            )
            showMessage(enabled ? "Power-save mode enabled" : "Power-save mode disabled", isError: false)
        } catch {
            powerSaveEnabled = !enabled
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }
        isLoading = false
    }

    func setFastBoot(enabled: Bool) async {
        guard fastBootLoaded else { return }
        isLoading = true
        let token = authManager.sessionToken
        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_mc.device.manager",
                method: "set_device_info",
                params: ["deviceInfoList": ["quicken_power_on": enabled ? "1" : "0"]]
            )
            showMessage(enabled ? "Fast boot enabled" : "Fast boot disabled", isError: false)
        } catch {
            fastBootEnabled = !enabled
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }
        isLoading = false
    }

    func reboot() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.power",
                method: "reboot",
                params: [:]
            )
            showMessage("Router is rebooting...", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func factoryReset() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_bsp.power",
                method: "factory_reset",
                params: [:]
            )
            showMessage("Factory reset initiated...", isError: false)
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

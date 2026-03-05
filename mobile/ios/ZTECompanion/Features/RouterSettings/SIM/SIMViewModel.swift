import SwiftUI

enum PinSheetAction {
    case verify        // unlock PIN-locked SIM
    case enableLock    // enable PIN lock on SIM
    case disableLock   // disable PIN lock on SIM
}

@Observable
@MainActor
final class SIMViewModel {
    var simInfo: SIMInfo = .empty
    var lockInfo: SIMLockInfo = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    // Sheet state
    var showChangePinSheet: Bool = false
    var showEnterPinSheet: Bool = false
    var showEnterPukSheet: Bool = false
    var showUnlockSheet: Bool = false
    var pinSheetAction: PinSheetAction = .verify

    // Form fields
    var pinInput: String = ""
    var oldPinInput: String = ""
    var newPinInput: String = ""
    var pukInput: String = ""
    var nckInput: String = ""

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    var isPinEnabled: Bool {
        simInfo.pinStatus == "1"
    }

    var isPinLocked: Bool {
        let sim = simInfo.simStatus.lowercased()
        let modem = simInfo.modemMainState.lowercased()
        return sim == "wait pin" || modem == "modem_waitpin"
    }

    var isPukLocked: Bool {
        let sim = simInfo.simStatus.lowercased()
        let modem = simInfo.modemMainState.lowercased()
        return sim == "wait puk" || modem == "modem_waitpuk"
    }

    func submitPin() async {
        switch pinSheetAction {
        case .verify:
            await verifyPin()
        case .enableLock:
            await changePinMode(enable: true)
            showEnterPinSheet = false
        case .disableLock:
            await changePinMode(enable: false)
            showEnterPinSheet = false
        }
    }

    func refresh() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken

        async let simTask = fetchSIMInfo(token: token)
        async let lockTask = fetchSIMLock(token: token)

        let (sim, lock) = await (simTask, lockTask)
        if let sim { simInfo = sim }
        if let lock { lockInfo = lock }

        isLoading = false
    }

    func changePinMode(enable: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "sim_change_pin_mode",
                params: [
                    "pin_num_m": pinInput,
                    "pin_mode": enable ? 1 : 0,
                    "pin_encode_flag": "0"
                ]
            )
            pinInput = ""
            showMessage("PIN lock \(enable ? "enabled" : "disabled")", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func changePin() async {
        guard oldPinInput.count >= 4, newPinInput.count >= 4 else {
            showMessage("PIN must be at least 4 digits", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "sim_change_pin",
                params: [
                    "pin_num": oldPinInput,
                    "new_pin_num": newPinInput,
                    "pin_encode_flag": "0"
                ]
            )
            oldPinInput = ""
            newPinInput = ""
            showChangePinSheet = false
            showMessage("PIN changed successfully", isError: false)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func verifyPin() async {
        guard pinInput.count >= 4 else {
            showMessage("PIN must be at least 4 digits", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "sim_verify_pin_puk",
                params: [
                    "pin_num": pinInput,
                    "puk_num": "",
                    "pin_encode_flag": "0"
                ]
            )
            pinInput = ""
            showEnterPinSheet = false
            showMessage("PIN verified", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func verifyPuk() async {
        guard pukInput.count >= 8 else {
            showMessage("PUK must be at least 8 digits", isError: true)
            return
        }
        guard newPinInput.count >= 4 else {
            showMessage("New PIN must be at least 4 digits", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "sim_verify_pin_puk",
                params: [
                    "pin_num": newPinInput,
                    "puk_num": pukInput,
                    "pin_encode_flag": "0"
                ]
            )
            pukInput = ""
            newPinInput = ""
            showEnterPukSheet = false
            showMessage("PUK verified, new PIN set", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func unlockSIM() async {
        guard !nckInput.isEmpty else {
            showMessage("Unlock code is required", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "set_simlock_nck",
                params: ["nck": nckInput]
            )
            nckInput = ""
            showUnlockSheet = false
            showMessage("SIM unlocked successfully", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    // MARK: - Private

    private func fetchSIMInfo(token: String) async -> SIMInfo? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "get_sim_info",
                params: [:]
            )
            return SIMParser.parseSIMInfo(data)
        } catch {
            showMessage("Failed to load SIM info: \(error.localizedDescription)", isError: true)
            return nil
        }
    }

    private func fetchSIMLock(token: String) async -> SIMLockInfo? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_zte_mdm.api",
                method: "get_simlock_available_trials",
                params: [:]
            )
            return SIMParser.parseSIMLock(data)
        } catch {
            return nil
        }
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

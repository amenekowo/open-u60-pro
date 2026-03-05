import SwiftUI

@Observable
@MainActor
final class APNViewModel {
    var config: APNConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var showFormSheet: Bool = false

    // Form state
    var formProfile: APNProfile = .empty
    var editingProfile: APNProfile?  // nil = adding, non-nil = editing
    var setAsDefault: Bool = false

    private let client: UbusClient
    private let authManager: AuthManager

    var isEditing: Bool { editingProfile != nil }

    /// The currently active APN name for display
    var activeAPNName: String? {
        let active = config.profiles.first(where: { $0.active })
            ?? config.autoProfiles.first(where: { $0.active })
        guard let active else { return nil }
        return active.name.isEmpty ? active.apn : active.name
    }

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func refresh() async {
        isLoading = true
        message = nil
        let token = authManager.sessionToken

        do {
            let (_, modeData) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_apn_mode",
                params: [:]
            )
            let mode = APNParser.parseMode(modeData)

            let (_, manuData) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_manu_apn_list",
                params: [:]
            )
            let profiles = APNParser.parseProfiles(manuData)

            let (_, autoData) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_auto_apn_list",
                params: [:]
            )
            let autoProfiles = APNParser.parseProfiles(autoData)

            config = APNConfig(mode: mode, profiles: profiles, autoProfiles: autoProfiles)
        } catch {
            showMessage("Failed to load APN: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func setMode(manual: Bool) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "set_apn_mode",
                params: ["apn_mode": manual ? 1 : 0]
            )
            showMessage("APN mode set to \(manual ? "manual" : "auto")", isError: false)
            config = APNConfig(
                mode: manual ? "1" : "0",
                profiles: config.profiles,
                autoProfiles: config.autoProfiles
            )
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    // MARK: - Form Actions

    func startAdd() {
        editingProfile = nil
        formProfile = .empty
        setAsDefault = false
        showFormSheet = true
    }

    func startEdit(_ profile: APNProfile) {
        editingProfile = profile
        formProfile = profile
        setAsDefault = profile.active
        showFormSheet = true
    }

    func saveAPN() async {
        guard !formProfile.name.isEmpty, !formProfile.apn.isEmpty else {
            showMessage("Name and APN are required", isError: true)
            return
        }

        // Duplicate name check (exclude self when editing)
        let isDuplicate = config.profiles.contains { p in
            p.name == formProfile.name && p.id != (editingProfile?.id ?? "")
        }
        if isDuplicate {
            showMessage("An APN with this name already exists", isError: true)
            return
        }

        if isEditing {
            await editAPN()
        } else {
            await addAPN()
        }
    }

    private func addAPN() async {
        isLoading = true
        let token = authManager.sessionToken

        let name = formProfile.name
        let apn = formProfile.apn
        let pdpType = formProfile.pdpType
        let authMode = formProfile.authMode
        let username = formProfile.username
        let password = formProfile.password
        let shouldSetDefault = setAsDefault

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "add_manu_apn",
                params: [
                    "profilename": name,
                    "wanapn": apn,
                    "pdpType": pdpType,
                    "pppAuthMode": authMode,
                    "username": username,
                    "password": password
                ]
            )

            if shouldSetDefault {
                // Refresh to get the new profile's ID, then activate it
                await refresh()
                if let newProfile = config.profiles.first(where: { $0.name == name && $0.apn == apn }) {
                    await activateAPN(newProfile)
                }
            }

            formProfile = .empty
            showFormSheet = false
            showMessage("APN added", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    private func editAPN() async {
        guard let editing = editingProfile else { return }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "modify_manu_apn",
                params: [
                    "profileId": editing.id,
                    "profilename": formProfile.name,
                    "wanapn": formProfile.apn,
                    "pdpType": formProfile.pdpType,
                    "pppAuthMode": formProfile.authMode,
                    "username": formProfile.username,
                    "password": formProfile.password
                ]
            )

            if setAsDefault && !editing.active {
                await activateAPN(editing)
            }

            editingProfile = nil
            showFormSheet = false
            showMessage("APN updated", isError: false)
            await refresh()
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func deleteAPN(_ profile: APNProfile) async {
        if profile.active {
            showMessage("Cannot delete the active APN", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "delete_manu_apn",
                params: ["profileId": profile.id]
            )
            showMessage("APN deleted", isError: false)
            config.profiles.removeAll { $0.id == profile.id }
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func activateAPN(_ profile: APNProfile) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "enable_manu_apn_id",
                params: ["profileId": profile.id]
            )
            showMessage("APN activated", isError: false)
            config.profiles = config.profiles.map { p in
                var updated = p
                updated.active = p.id == profile.id
                return updated
            }
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

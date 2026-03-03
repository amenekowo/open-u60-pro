import SwiftUI

@Observable
@MainActor
final class APNViewModel {
    var config: APNConfig = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false
    var showAddSheet: Bool = false

    // New APN form
    var newProfile: APNProfile = .empty

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
            let (_, modeData) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_apn_mode",
                params: [:]
            )
            let mode = APNParser.parseMode(modeData)

            let (_, listData) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "get_manu_apn_list",
                params: [:]
            )
            let profiles = APNParser.parseProfiles(listData)

            config = APNConfig(mode: mode, profiles: profiles)
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
                params: ["apn_mode": manual ? "1" : "0"]
            )
            showMessage("APN mode set to \(manual ? "manual" : "auto")", isError: false)
            config = APNConfig(mode: manual ? "1" : "0", profiles: config.profiles)
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func addAPN() async {
        guard !newProfile.name.isEmpty, !newProfile.apn.isEmpty else {
            showMessage("Name and APN are required", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "add_manu_apn",
                params: [
                    "name": newProfile.name,
                    "apn": newProfile.apn,
                    "pdp_type": newProfile.pdpType,
                    "auth_mode": newProfile.authMode,
                    "username": newProfile.username,
                    "password": newProfile.password
                ]
            )
            newProfile = .empty
            showAddSheet = false
            showMessage("APN added", isError: false)
            config.profiles.append(APNProfile(id: UUID().uuidString, name: newProfile.name, apn: newProfile.apn,
                                              pdpType: newProfile.pdpType, authMode: newProfile.authMode,
                                              username: newProfile.username, password: newProfile.password,
                                              active: false))
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func deleteAPN(_ profile: APNProfile) async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_apn_object",
                method: "delete_manu_apn",
                params: ["id": profile.id]
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
                params: ["id": profile.id]
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

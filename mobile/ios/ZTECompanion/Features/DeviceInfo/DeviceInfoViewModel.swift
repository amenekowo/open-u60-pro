import SwiftUI

@Observable
@MainActor
final class DeviceInfoViewModel {
    var identity: DeviceIdentity = .empty
    var operatorInfo: OperatorInfo = .empty
    var isLoading: Bool = false
    var error: String?

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func refresh() async {
        isLoading = true
        error = nil
        let token = authManager.sessionToken

        async let simTask = fetchSIMInfo(token: token)
        async let imeiTask = fetchIMEI(token: token)
        async let wanTask = fetchWANStatus(token: token)
        async let wan6Task = fetchWAN6Status(token: token)
        async let lanTask = fetchLANStatus(token: token)
        async let signalTask = fetchSignalInfo(token: token)

        let (simInfo, imeiData, wanStatus, wan6Status, lanStatus, signalInfo) =
            await (simTask, imeiTask, wanTask, wan6Task, lanTask, signalTask)

        identity = DeviceParser.parseIdentity(
            simInfo: simInfo ?? [:],
            imeiData: imeiData ?? [:],
            wanStatus: wanStatus ?? [:],
            wan6Status: wan6Status ?? [:],
            lanStatus: lanStatus ?? [:]
        )
        if let signalInfo {
            operatorInfo = signalInfo
        }
        isLoading = false
    }

    private func fetchSIMInfo(token: String) async -> [String: Any]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zwrt_zte_mdm.api",
                method: "get_sim_info", params: [:]
            )
            return data
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    private func fetchIMEI(token: String) async -> [String: Any]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zwrt_zte_mdm.api",
                method: "get_imei", params: [:]
            )
            return data
        } catch { return nil }
    }

    private func fetchWANStatus(token: String) async -> [String: Any]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "network.interface.zte_wan",
                method: "status", params: [:]
            )
            return data
        } catch { return nil }
    }

    private func fetchWAN6Status(token: String) async -> [String: Any]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "network.interface.zte_wan6",
                method: "status", params: [:]
            )
            return data
        } catch { return nil }
    }

    private func fetchLANStatus(token: String) async -> [String: Any]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "network.interface.lan",
                method: "status", params: [:]
            )
            return data
        } catch { return nil }
    }

    private func fetchSignalInfo(token: String) async -> OperatorInfo? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_get_netinfo", params: [:]
            )
            let (_, _, _, opInfo) = SignalParser.parseNetInfo(data)
            return opInfo
        } catch { return nil }
    }
}

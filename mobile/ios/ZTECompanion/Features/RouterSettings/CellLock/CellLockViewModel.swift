import SwiftUI

@Observable
@MainActor
final class CellLockViewModel {
    var status: CellLockStatus = .empty
    var neighbors: [NeighborCell] = []
    var isLoading: Bool = false
    var isScanning: Bool = false
    var message: String?
    var messageIsError: Bool = false

    // NR lock fields
    var nrPCI: String = ""
    var nrEARFCN: String = ""
    var nrBand: String = ""

    // LTE lock fields
    var ltePCI: String = ""
    var lteEARFCN: String = ""

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
                object: "zte_nwinfo_api",
                method: "nwinfo_get_netinfo",
                params: [:]
            )
            status = CellLockParser.parse(data)
        } catch {
            showMessage("Failed to load cell info: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func lockNR() async {
        guard !nrPCI.isEmpty, !nrEARFCN.isEmpty else {
            showMessage("PCI and EARFCN are required", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            var params: [String: Any] = ["pci": nrPCI, "earfcn": nrEARFCN]
            if !nrBand.isEmpty { params["band"] = nrBand }

            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_lock_nr_cell",
                params: params
            )
            showMessage("NR cell locked", isError: false)
            status.locked = true
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func lockLTE() async {
        guard !ltePCI.isEmpty, !lteEARFCN.isEmpty else {
            showMessage("PCI and EARFCN are required", isError: true)
            return
        }

        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_lock_lte_cell",
                params: ["pci": ltePCI, "earfcn": lteEARFCN]
            )
            showMessage("LTE cell locked", isError: false)
            status.locked = true
        } catch {
            showMessage("Failed: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func scanNeighbors() async {
        isScanning = true
        neighbors = []
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_scan_nbr",
                params: [:]
            )

            // Poll for results
            try await Task.sleep(for: .seconds(3))

            // Fetch NR neighbors
            if let nrCells = try? await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_nr5g_nbr_contents",
                params: [:]
            ) {
                neighbors += CellLockParser.parseNeighbors(nrCells.1, type: "NR")
            }

            // Fetch LTE neighbors
            if let lteCells = try? await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_lte_nbr_contents",
                params: [:]
            ) {
                neighbors += CellLockParser.parseNeighbors(lteCells.1, type: "LTE")
            }

            if neighbors.isEmpty {
                showMessage("No neighbors found", isError: false)
            } else {
                showMessage("Found \(neighbors.count) neighbor cell(s)", isError: false)
            }
        } catch {
            showMessage("Scan failed: \(error.localizedDescription)", isError: true)
        }

        isScanning = false
    }

    func unlock() async {
        isLoading = true
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_reset_band_cell_setting",
                params: [:]
            )
            showMessage("Cell lock reset", isError: false)
            status.locked = false
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

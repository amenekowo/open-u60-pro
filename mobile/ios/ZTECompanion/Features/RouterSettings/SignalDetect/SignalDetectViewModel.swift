import SwiftUI

@Observable
@MainActor
final class SignalDetectViewModel {
    var status: SignalDetectStatus = .empty
    var isLoading: Bool = false
    var message: String?
    var messageIsError: Bool = false

    private let client: UbusClient
    private let authManager: AuthManager
    private var pollTask: Task<Void, Never>?

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func startDetection() async {
        isLoading = true
        message = nil
        status.results = []
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_start_detect_signal_quality",
                params: [:]
            )
            status.running = true
            showMessage("Detection started", isError: false)
            startPolling()
        } catch {
            showMessage("Failed to start: \(error.localizedDescription)", isError: true)
        }

        isLoading = false
    }

    func stopDetection() async {
        stopPolling()
        let token = authManager.sessionToken

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_end_detect_signal_quality",
                params: [:]
            )
            status.running = false
            showMessage("Detection stopped", isError: false)
            await fetchResults()
        } catch {
            showMessage("Failed to stop: \(error.localizedDescription)", isError: true)
        }
    }

    func fetchResults() async {
        let token = authManager.sessionToken

        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_detect_quality_recorder",
                params: [:]
            )
            status.results = SignalDetectParser.parseResults(data)
        } catch {
            // Results may not be available yet
        }
    }

    private func startPolling() {
        stopPolling()
        pollTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                await pollProgress()
                if !status.running { break }
            }
        }
    }

    private func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }

    private func pollProgress() async {
        let token = authManager.sessionToken

        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zte_nwinfo_api",
                method: "nwinfo_get_progress_and_quality",
                params: [:]
            )
            let progressStatus = SignalDetectParser.parseProgress(data)
            status.progress = progressStatus.progress
            if progressStatus.progress >= 100 {
                status.running = false
                stopPolling()
                await fetchResults()
                showMessage("Detection complete", isError: false)
            }
        } catch {
            // Continue polling
        }
    }

    private func showMessage(_ text: String, isError: Bool) {
        message = text
        messageIsError = isError
    }
}

import Foundation

/// JSON-RPC 2.0 client for the ZTE ubus API.
@MainActor
final class UbusClient {
    static let anonSession = "00000000000000000000000000000000"

    private let session: URLSession
    private var requestID: Int = 0

    var gatewayIP: String {
        didSet { UserDefaults.standard.set(gatewayIP, forKey: "gateway_ip") }
    }

    init(gatewayIP: String = "192.168.0.1") {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 15
        self.session = URLSession(configuration: config)
        self.gatewayIP = gatewayIP
    }

    private var baseURL: URL? {
        let ts = Int(Date().timeIntervalSince1970 * 1000)
        return URL(string: "http://\(gatewayIP)/ubus/?t=\(ts)")
    }

    private func nextID() -> Int {
        requestID += 1
        return requestID
    }

    /// Perform a ubus JSON-RPC call and return the raw result array.
    func call(
        sessionToken: String,
        object: String,
        method: String,
        params: [String: Any] = [:]
    ) async throws -> (Int, [String: Any]) {
        guard let url = baseURL else { throw UbusError.invalidURL }

        let payload: [[String: Any]] = [
            [
                "jsonrpc": "2.0",
                "id": nextID(),
                "method": "call",
                "params": [sessionToken, object, method, params] as [Any]
            ]
        ]

        let body = try JSONSerialization.data(withJSONObject: payload)

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await session.data(for: request)
        } catch let error as URLError where error.code == .timedOut {
            throw UbusError.timeout
        } catch {
            throw UbusError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse,
              (200...299).contains(httpResponse.statusCode) else {
            throw UbusError.serverUnreachable
        }

        guard let jsonArray = try JSONSerialization.jsonObject(with: data) as? [[String: Any]],
              let first = jsonArray.first,
              let result = first["result"] as? [Any] else {
            throw UbusError.decodingError("Unexpected response format")
        }

        let statusCode = (result.first as? Int) ?? -1
        let resultData = (result.count > 1 ? result[1] as? [String: Any] : nil) ?? [:]

        if statusCode != 0 {
            throw UbusError.requestFailed(statusCode)
        }

        return (statusCode, resultData)
    }

    /// Perform an anonymous ubus call (no authentication required).
    func callAnon(
        object: String,
        method: String,
        params: [String: Any] = [:]
    ) async throws -> (Int, [String: Any]) {
        try await call(
            sessionToken: Self.anonSession,
            object: object,
            method: method,
            params: params
        )
    }

    /// Check if the gateway is reachable.
    func ping() async -> Bool {
        guard let url = URL(string: "http://\(gatewayIP)/") else { return false }
        var request = URLRequest(url: url)
        request.httpMethod = "HEAD"
        request.timeoutInterval = 3
        do {
            let (_, response) = try await session.data(for: request)
            return (response as? HTTPURLResponse)?.statusCode != nil
        } catch {
            return false
        }
    }
}

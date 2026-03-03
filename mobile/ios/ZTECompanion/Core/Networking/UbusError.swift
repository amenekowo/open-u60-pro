import Foundation

enum UbusError: LocalizedError {
    case invalidURL
    case notAuthenticated
    case authenticationFailed(String)
    case requestFailed(Int)
    case networkError(Error)
    case decodingError(String)
    case timeout
    case serverUnreachable

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Invalid gateway URL"
        case .notAuthenticated:
            return "Not authenticated. Please log in."
        case .authenticationFailed(let reason):
            return "Authentication failed: \(reason)"
        case .requestFailed(let code):
            return "Request failed with ubus status code \(code)"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        case .decodingError(let detail):
            return "Failed to decode response: \(detail)"
        case .timeout:
            return "Request timed out"
        case .serverUnreachable:
            return "Cannot reach the gateway"
        }
    }
}

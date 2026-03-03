import Foundation
import CryptoKit
import Observation

/// Manages authentication state and session tokens for the ZTE ubus API.
@Observable
@MainActor
final class AuthManager {
    enum AuthState: Equatable {
        case idle
        case authenticating
        case authenticated
        case failed(String)
    }

    var state: AuthState = .idle
    var sessionToken: String = UbusClient.anonSession

    private let client: UbusClient
    private let maxSaltRetries = 3

    var isAuthenticated: Bool { state == .authenticated }

    init(client: UbusClient) {
        self.client = client
    }

    /// Fetch the login salt from the router.
    /// The salt field is named `zte_web_sault` (ZTE typo). Retries up to 3 times since it can be flaky.
    private func fetchSalt() async throws -> String {
        var lastError: Error?
        for attempt in 1...maxSaltRetries {
            do {
                let (_, data) = try await client.callAnon(
                    object: "zwrt_web",
                    method: "web_login_info",
                    params: [:]
                )
                if let salt = data["zte_web_sault"] as? String, !salt.isEmpty {
                    return salt
                }
                lastError = UbusError.authenticationFailed("Empty salt returned")
            } catch {
                lastError = error
            }
            if attempt < maxSaltRetries {
                try await Task.sleep(nanoseconds: 500_000_000)
            }
        }
        throw lastError ?? UbusError.authenticationFailed("Failed to fetch salt")
    }

    /// Compute the ZTE double-SHA256 password hash.
    /// Formula: UPPER(SHA256(UPPER(SHA256(password)) + salt))
    private func hashPassword(_ password: String, salt: String) -> String {
        let passData = Data(password.utf8)
        let firstHash = SHA256.hash(data: passData)
        let firstHex = firstHash.map { String(format: "%02x", $0) }.joined().uppercased()
        let combined = Data((firstHex + salt).utf8)
        let secondHash = SHA256.hash(data: combined)
        let secondHex = secondHash.map { String(format: "%02x", $0) }.joined().uppercased()
        return secondHex
    }

    /// Perform full login: fetch salt, hash password, authenticate.
    func login(password: String) async {
        state = .authenticating
        do {
            let salt = try await fetchSalt()
            let hashedPassword = hashPassword(password, salt: salt)

            let (_, data) = try await client.callAnon(
                object: "zwrt_web",
                method: "web_login",
                params: ["password": hashedPassword]
            )

            guard let token = data["ubus_rpc_session"] as? String,
                  token != UbusClient.anonSession,
                  !token.isEmpty else {
                state = .failed("Invalid credentials")
                return
            }

            sessionToken = token
            state = .authenticated
        } catch let error as UbusError {
            state = .failed(error.localizedDescription)
        } catch {
            state = .failed(error.localizedDescription)
        }
    }

    /// Clear the current session.
    func logout() {
        sessionToken = UbusClient.anonSession
        state = .idle
    }

    /// Re-authenticate silently using stored credentials.
    /// Does NOT change auth state on failure — the user stays logged in.
    func reauthenticate() async -> Bool {
        guard let password = KeychainHelper.load(key: "router_password") else { return false }
        do {
            let salt = try await fetchSalt()
            let hashedPassword = hashPassword(password, salt: salt)
            let (_, data) = try await client.callAnon(
                object: "zwrt_web",
                method: "web_login",
                params: ["password": hashedPassword]
            )
            guard let token = data["ubus_rpc_session"] as? String,
                  token != UbusClient.anonSession,
                  !token.isEmpty else {
                return false
            }
            sessionToken = token
            state = .authenticated  // refresh state in case it drifted
            return true
        } catch {
            return false
        }
    }
}

// MARK: - Keychain Helper

enum KeychainHelper {
    static func save(key: String, value: String) {
        let data = Data(value.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: key,
            kSecAttrService as String: "com.ztecompanion.app"
        ]
        SecItemDelete(query as CFDictionary)
        var addQuery = query
        addQuery[kSecValueData as String] = data
        SecItemAdd(addQuery as CFDictionary, nil)
    }

    static func load(key: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: key,
            kSecAttrService as String: "com.ztecompanion.app",
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: key,
            kSecAttrService as String: "com.ztecompanion.app"
        ]
        SecItemDelete(query as CFDictionary)
    }
}

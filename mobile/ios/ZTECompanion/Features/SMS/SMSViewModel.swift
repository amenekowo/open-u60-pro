import SwiftUI
import os

private let logger = Logger(subsystem: "com.zte.companion", category: "SMS")

@Observable
@MainActor
final class SMSViewModel {
    var conversations: [SMSConversation] = []
    var allMessages: [SMSMessage] = []
    var capacity: SMSCapacity = .empty
    var isLoading = false
    var isSending = false
    var error: String?

    private let client: UbusClient
    private let authManager: AuthManager

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    // MARK: - Fetch

    func refresh() async {
        isLoading = true
        error = nil
        var token = authManager.sessionToken

        // Fetch messages — retry once on session expired (code 6)
        var messages = await fetchMessages(token: token)
        if messages == nil, await authManager.reauthenticate() {
            token = authManager.sessionToken
            messages = await fetchMessages(token: token)
        }

        if let messages {
            allMessages = messages
            conversations = SMSParser.groupIntoConversations(messages)
        }

        // Fetch capacity in parallel (non-critical)
        if let cap = await fetchCapacity(token: token) {
            capacity = cap
        }

        isLoading = false
    }

    private func fetchMessages(token: String) async -> [SMSMessage]? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_wms",
                method: "zte_libwms_get_sms_data",
                params: [
                    "page": 0,
                    "data_per_page": 500,
                    "mem_store": 1,
                    "tags": 10,
                    "order_by": "order by id desc"
                ]
            )
            return SMSParser.parseMessages(data)
        } catch {
            logger.error("fetchMessages: \(error.localizedDescription)")
            self.error = error.localizedDescription
            return nil
        }
    }

    private func fetchCapacity(token: String) async -> SMSCapacity? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token,
                object: "zwrt_wms",
                method: "zwrt_wms_get_wms_capacity",
                params: [:]
            )
            return SMSParser.parseCapacity(data)
        } catch {
            logger.warning("fetchCapacity: \(error.localizedDescription)")
            return nil
        }
    }

    // MARK: - Send

    func sendSMS(to number: String, message: String) async -> Bool {
        isSending = true
        error = nil
        let token = authManager.sessionToken

        let encodeType = SMSParser.getEncodeType(message)
        // Web UI always UCS-2 encodes the body via encodeMessage()
        let body = SMSParser.encodeUCS2Hex(message)
        let smsTime = SMSParser.formatSMSTime()

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_wms",
                method: "zte_libwms_send_sms",
                params: [
                    "number": number,
                    "sms_time": smsTime,
                    "message_body": body,
                    "id": "-1",
                    "encode_type": encodeType
                ]
            )
            logger.info("SMS sent to \(number)")
            isSending = false
            await refresh()
            return true
        } catch {
            logger.error("sendSMS: \(error.localizedDescription)")
            self.error = "Failed to send: \(error.localizedDescription)"
            isSending = false
            return false
        }
    }

    // MARK: - Delete

    func deleteMessages(ids: [Int]) async {
        let token = authManager.sessionToken
        let idStr = ids.map(String.init).joined(separator: ";") + ";"

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_wms",
                method: "zwrt_wms_delete_sms",
                params: ["id": idStr]
            )
            logger.info("Deleted SMS ids: \(idStr)")
            await refresh()
        } catch {
            logger.error("deleteMessages: \(error.localizedDescription)")
            self.error = "Delete failed: \(error.localizedDescription)"
        }
    }

    func deleteConversation(_ conversation: SMSConversation) async {
        let ids = conversation.messages.map(\.id)
        await deleteMessages(ids: ids)
    }

    // MARK: - Mark Read

    func markAsRead(ids: [Int]) async {
        guard !ids.isEmpty else { return }
        let token = authManager.sessionToken
        let idStr = ids.map(String.init).joined(separator: ";") + ";"

        do {
            let (_, _) = try await client.call(
                sessionToken: token,
                object: "zwrt_wms",
                method: "zwrt_wms_modify_tag",
                params: ["id": idStr, "tag": 0]
            )
            // Update local state without full refresh
            for i in allMessages.indices where ids.contains(allMessages[i].id) {
                let msg = allMessages[i]
                allMessages[i] = SMSMessage(
                    id: msg.id, number: msg.number, content: msg.content,
                    date: msg.date, tag: .read, groupId: msg.groupId, memStore: msg.memStore
                )
            }
            conversations = SMSParser.groupIntoConversations(allMessages)
        } catch {
            logger.warning("markAsRead: \(error.localizedDescription)")
        }
    }
}

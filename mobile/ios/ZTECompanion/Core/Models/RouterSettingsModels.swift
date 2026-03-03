import Foundation

// MARK: - Network Mode

struct NetworkModeConfig: Equatable {
    var netSelect: String       // WL_AND_5G, Only_5G, Only_LTE, Only_WCDMA, Only_GSM

    static let empty = NetworkModeConfig(netSelect: "WL_AND_5G")

    static let netSelectOptions: [(label: String, value: String)] = [
        ("Auto (5G + LTE)", "WL_AND_5G"),
        ("5G Only", "Only_5G"),
        ("LTE Only", "Only_LTE"),
        ("3G Only", "Only_WCDMA"),
        ("2G Only", "Only_GSM")
    ]
}

enum NetworkModeParser {
    static func parse(_ data: [String: Any]) -> NetworkModeConfig {
        NetworkModeConfig(
            netSelect: data["net_select"] as? String ?? ""
        )
    }
}

// MARK: - Cell Lock

struct CellLockStatus: Equatable {
    var nrPCI: String
    var nrEARFCN: String
    var nrBand: String
    var ltePCI: String
    var lteEARFCN: String
    var locked: Bool

    static let empty = CellLockStatus(
        nrPCI: "", nrEARFCN: "", nrBand: "",
        ltePCI: "", lteEARFCN: "", locked: false
    )
}

struct NeighborCell: Equatable, Identifiable {
    let id = UUID()
    var pci: String
    var earfcn: String
    var band: String
    var rsrp: String
    var type: String    // "NR" or "LTE"
}

enum CellLockParser {
    static func parse(_ data: [String: Any]) -> CellLockStatus {
        CellLockStatus(
            nrPCI: data["nr_pci"] as? String ?? data["nr5g_pci"] as? String ?? "",
            nrEARFCN: data["nr_earfcn"] as? String ?? data["nr5g_earfcn"] as? String ?? "",
            nrBand: data["nr_band"] as? String ?? data["nr5g_band"] as? String ?? "",
            ltePCI: data["lte_pci"] as? String ?? "",
            lteEARFCN: data["lte_earfcn"] as? String ?? "",
            locked: asBool(data["cell_lock_status"])
        )
    }

    static func parseNeighbors(_ data: [String: Any], type: String) -> [NeighborCell] {
        guard let cells = data["cell_list"] as? [[String: Any]] else { return [] }
        return cells.map { cell in
            NeighborCell(
                pci: cell["pci"] as? String ?? "",
                earfcn: cell["earfcn"] as? String ?? "",
                band: cell["band"] as? String ?? "",
                rsrp: cell["rsrp"] as? String ?? "",
                type: type
            )
        }
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - STC (Smart Tower Connect)

struct STCConfig: Equatable {
    var lteCollectTimer: String
    var nrsaCollectTimer: String
    var lteWhitelistMax: String
    var nrsaWhitelistMax: String
    var enabled: Bool

    static let empty = STCConfig(
        lteCollectTimer: "", nrsaCollectTimer: "",
        lteWhitelistMax: "", nrsaWhitelistMax: "",
        enabled: false
    )
}

enum STCParser {
    static func parseParams(_ data: [String: Any]) -> STCConfig {
        STCConfig(
            lteCollectTimer: data["lte_collect_timer"] as? String ?? "",
            nrsaCollectTimer: data["nrsa_collect_timer"] as? String ?? "",
            lteWhitelistMax: data["lte_whitelist_max"] as? String ?? "",
            nrsaWhitelistMax: data["nrsa_whitelist_max"] as? String ?? "",
            enabled: asBool(data["stc_enable"])
        )
    }

    static func parseStatus(_ data: [String: Any], into config: STCConfig) -> STCConfig {
        var updated = config
        updated.enabled = asBool(data["stc_enable"]) || asBool(data["status"])
        return updated
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - Signal Detect

struct SignalDetectStatus: Equatable {
    var progress: Int
    var running: Bool
    var results: [SignalQualityResult]

    static let empty = SignalDetectStatus(progress: 0, running: false, results: [])
}

struct SignalQualityResult: Equatable, Identifiable {
    let id = UUID()
    var band: String
    var earfcn: String
    var pci: String
    var rsrp: String
    var rsrq: String
    var sinr: String
    var type: String
}

enum SignalDetectParser {
    static func parseProgress(_ data: [String: Any]) -> SignalDetectStatus {
        SignalDetectStatus(
            progress: asInt(data["progress"]) ?? 0,
            running: asBool(data["running"]),
            results: []
        )
    }

    static func parseResults(_ data: [String: Any]) -> [SignalQualityResult] {
        guard let records = data["record_list"] as? [[String: Any]] else { return [] }
        return records.map { record in
            SignalQualityResult(
                band: record["band"] as? String ?? "",
                earfcn: record["earfcn"] as? String ?? "",
                pci: record["pci"] as? String ?? "",
                rsrp: record["rsrp"] as? String ?? "",
                rsrq: record["rsrq"] as? String ?? "",
                sinr: record["sinr"] as? String ?? "",
                type: record["type"] as? String ?? ""
            )
        }
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }

    private static func asInt(_ val: Any?) -> Int? {
        if let i = val as? Int { return i }
        if let s = val as? String { return Int(s) }
        if let d = val as? Double { return Int(d) }
        return nil
    }
}

// MARK: - Mobile Network

struct MobileNetworkConfig: Equatable {
    var connectMode: Int        // 0=manual, 1=auto
    var roamEnable: Int         // 0=off, 1=on
    var netSelectMode: String   // "auto_select" / "manual_select"
    var currentAPN: String
    var operators: [NetworkOperator]
    var scanStatus: String      // "", "scanning", "done"

    var isAutoConnect: Bool { connectMode == 1 }
    var isRoamingEnabled: Bool { roamEnable == 1 }
    var isAutoNetSelect: Bool { netSelectMode == "auto_select" }

    static let empty = MobileNetworkConfig(
        connectMode: 1, roamEnable: 0, netSelectMode: "auto_select",
        currentAPN: "", operators: [], scanStatus: ""
    )
}

struct NetworkOperator: Equatable, Identifiable {
    let id = UUID()
    var name: String
    var mccMnc: String
    var rat: String
    var status: String  // "available", "current", "forbidden"
}

enum MobileNetworkParser {
    static func parseWWAN(_ data: [String: Any]) -> (connectMode: Int, roamEnable: Int) {
        let connectMode = asInt(data["connect_mode"]) ?? 1
        let roamEnable = asInt(data["roam_enable"]) ?? 0
        return (connectMode, roamEnable)
    }

    static func parseNetInfo(_ data: [String: Any]) -> String {
        data["net_select_mode"] as? String ?? "auto_select"
    }

    static func parseScanStatus(_ data: [String: Any]) -> String {
        data["m_netselect_status"] as? String ?? ""
    }

    static func parseScanResults(_ data: [String: Any]) -> [NetworkOperator] {
        guard let list = data["m_netselect_contents"] as? [[String: Any]] else { return [] }
        return list.map { item in
            NetworkOperator(
                name: item["name"] as? String ?? item["operator_name"] as? String ?? "",
                mccMnc: item["mcc_mnc"] as? String ?? item["plmn"] as? String ?? "",
                rat: item["rat"] as? String ?? "",
                status: item["status"] as? String ?? "available"
            )
        }
    }

    static func parseRegisterResult(_ data: [String: Any]) -> String {
        data["m_netselect_result"] as? String ?? ""
    }

    static func parseCurrentAPN(_ modeData: [String: Any], _ listData: [String: Any]) -> String {
        let profiles = APNParser.parseProfiles(listData)
        if let active = profiles.first(where: { $0.active }) {
            return active.name.isEmpty ? active.apn : active.name
        }
        return profiles.first?.name ?? ""
    }

    private static func asInt(_ val: Any?) -> Int? {
        if let i = val as? Int { return i }
        if let s = val as? String { return Int(s) }
        if let d = val as? Double { return Int(d) }
        return nil
    }
}

// MARK: - APN

struct APNConfig: Equatable {
    var mode: String            // "0" = auto, "1" = manual
    var profiles: [APNProfile]

    var isManual: Bool { mode == "1" }

    static let empty = APNConfig(mode: "", profiles: [])
}

struct APNProfile: Equatable, Identifiable {
    let id: String
    var name: String
    var apn: String
    var pdpType: String
    var authMode: String
    var username: String
    var password: String
    var active: Bool

    static let empty = APNProfile(
        id: "", name: "", apn: "", pdpType: "IPV4V6",
        authMode: "none", username: "", password: "", active: false
    )

    static let pdpTypeOptions = ["IPV4", "IPV6", "IPV4V6"]
    static let authModeOptions = ["none", "pap", "chap", "pap_chap"]
}

enum APNParser {
    static func parseMode(_ data: [String: Any]) -> String {
        data["apn_mode"] as? String ?? "0"
    }

    static func parseProfiles(_ data: [String: Any]) -> [APNProfile] {
        guard let list = data["apn_list"] as? [[String: Any]] else { return [] }
        return list.enumerated().map { index, item in
            APNProfile(
                id: item["id"] as? String ?? "\(index)",
                name: item["name"] as? String ?? "",
                apn: item["apn"] as? String ?? "",
                pdpType: item["pdp_type"] as? String ?? "IPV4V6",
                authMode: item["auth_mode"] as? String ?? "none",
                username: item["username"] as? String ?? "",
                password: item["password"] as? String ?? "",
                active: asBool(item["active"])
            )
        }
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - WiFi

struct WiFiConfig: Equatable {
    var ssid2g: String
    var ssid5g: String
    var key2g: String
    var key5g: String
    var channel2g: String
    var channel5g: String
    var txpower2g: String
    var txpower5g: String
    var encryption2g: String
    var encryption5g: String
    var wifiOnOff: Bool
    var hidden2g: Bool
    var hidden5g: Bool

    static let empty = WiFiConfig(
        ssid2g: "", ssid5g: "", key2g: "", key5g: "",
        channel2g: "auto", channel5g: "auto",
        txpower2g: "100", txpower5g: "100",
        encryption2g: "psk2+ccmp", encryption5g: "psk2+ccmp",
        wifiOnOff: true, hidden2g: false, hidden5g: false
    )

    static let channelOptions2g = ["auto", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"]
    static let channelOptions5g = ["auto", "36", "40", "44", "48", "52", "56", "60", "64", "100", "104", "108", "112", "116", "120", "124", "128", "132", "136", "140", "149", "153", "157", "161", "165"]
    static let txpowerOptions = ["25", "50", "75", "100"]
    static let encryptionOptions = ["none", "psk+tkip", "psk+ccmp", "psk2+ccmp", "psk-mixed+ccmp"]
}

enum WiFiParser {
    static func parse(_ data: [String: Any]) -> WiFiConfig {
        WiFiConfig(
            ssid2g: data["ssid_2g"] as? String ?? "",
            ssid5g: data["ssid_5g"] as? String ?? "",
            key2g: data["key_2g"] as? String ?? "",
            key5g: data["key_5g"] as? String ?? "",
            channel2g: data["channel_2g"] as? String ?? "auto",
            channel5g: data["channel_5g"] as? String ?? "auto",
            txpower2g: data["txpower_2g"] as? String ?? "100",
            txpower5g: data["txpower_5g"] as? String ?? "100",
            encryption2g: data["encryption_2g"] as? String ?? "psk2+ccmp",
            encryption5g: data["encryption_5g"] as? String ?? "psk2+ccmp",
            wifiOnOff: asBool(data["wifi_onoff"]),
            hidden2g: asBool(data["hidden_2g"]),
            hidden5g: asBool(data["hidden_5g"])
        )
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - LAN/DHCP

struct LANConfig: Equatable {
    var lanIP: String
    var netmask: String
    var dhcpEnabled: Bool
    var dhcpStart: String
    var dhcpEnd: String
    var dhcpLeaseTime: String

    static let empty = LANConfig(
        lanIP: "", netmask: "", dhcpEnabled: false,
        dhcpStart: "", dhcpEnd: "", dhcpLeaseTime: ""
    )
}

enum LANParser {
    static func parse(_ data: [String: Any]) -> LANConfig {
        LANConfig(
            lanIP: data["lan_ipaddr"] as? String ?? "",
            netmask: data["lan_netmask"] as? String ?? "",
            dhcpEnabled: asBool(data["dhcp_enable"]),
            dhcpStart: data["dhcp_start"] as? String ?? "",
            dhcpEnd: data["dhcp_end"] as? String ?? "",
            dhcpLeaseTime: data["dhcp_lease_time"] as? String ?? ""
        )
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - QoS

struct QoSConfig: Equatable {
    var enabled: Bool

    static let empty = QoSConfig(enabled: false)
}

enum QoSParser {
    static func parse(_ data: [String: Any]) -> QoSConfig {
        QoSConfig(enabled: asBool(data["qos_switch"]))
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - VPN Passthrough

struct VPNPassthroughConfig: Equatable {
    var l2tp: Bool
    var pptp: Bool
    var ipsec: Bool

    static let empty = VPNPassthroughConfig(l2tp: false, pptp: false, ipsec: false)
}

enum VPNPassthroughParser {
    static func parse(_ data: [String: Any]) -> VPNPassthroughConfig {
        VPNPassthroughConfig(
            l2tp: asBool(data["l2tp_passthrough"]),
            pptp: asBool(data["pptp_passthrough"]),
            ipsec: asBool(data["ipsec_passthrough"])
        )
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

// MARK: - Scheduled Reboot

struct ScheduleRebootConfig: Equatable {
    var enabled: Bool
    var time: String        // "HH:MM"
    var days: String        // comma-separated day numbers

    static let empty = ScheduleRebootConfig(enabled: false, time: "03:00", days: "")

    static let dayOptions: [(label: String, value: String)] = [
        ("Mon", "1"), ("Tue", "2"), ("Wed", "3"), ("Thu", "4"),
        ("Fri", "5"), ("Sat", "6"), ("Sun", "0")
    ]
}

enum ScheduleRebootParser {
    static func parse(_ data: [String: Any]) -> ScheduleRebootConfig {
        ScheduleRebootConfig(
            enabled: asBool(data["auto_reboot_enable"]),
            time: data["auto_reboot_time"] as? String ?? "03:00",
            days: data["auto_reboot_days"] as? String ?? ""
        )
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }
}

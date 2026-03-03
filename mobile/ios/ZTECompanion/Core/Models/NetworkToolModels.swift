import Foundation

// MARK: - DNS

struct DNSConfig: Equatable {
    var wanDnsMode: String       // "auto" or "manual"
    var primaryDns: String
    var secondaryDns: String
    var ipv6PrimaryDns: String
    var ipv6SecondaryDns: String
    var ipv6DnsMode: String

    var isManual: Bool { wanDnsMode == "manual" }

    static let empty = DNSConfig(
        wanDnsMode: "", primaryDns: "", secondaryDns: "",
        ipv6PrimaryDns: "", ipv6SecondaryDns: "", ipv6DnsMode: ""
    )
}

enum DNSParser {
    static func parse(_ data: [String: Any]) -> DNSConfig {
        DNSConfig(
            wanDnsMode: data["wan_dns_mode"] as? String ?? "",
            primaryDns: data["wan_prefer_dns_manual"] as? String ?? "",
            secondaryDns: data["wan_standby_dns_manual"] as? String ?? "",
            ipv6PrimaryDns: data["ipv6_wan_prefer_dns_manual"] as? String ?? "",
            ipv6SecondaryDns: data["ipv6_wan_standby_dns_manual"] as? String ?? "",
            ipv6DnsMode: data["ipv6_wan_dns_mode"] as? String ?? ""
        )
    }
}

// MARK: - Firewall

struct FirewallConfig: Equatable {
    var enabled: Bool
    var nat: Bool
    var dmzEnabled: Bool
    var dmzHost: String
    var level: String            // "low", "medium", "high"
    var wanPingFilter: Bool
    var portForwardEnabled: Bool

    static let empty = FirewallConfig(
        enabled: false, nat: false, dmzEnabled: false, dmzHost: "",
        level: "medium", wanPingFilter: false, portForwardEnabled: false
    )
}

struct PortForwardRule: Equatable, Identifiable {
    let id: String
    var name: String
    var protocol_: String
    var wanPort: String
    var lanIP: String
    var lanPort: String
    var enabled: Bool
}

struct FilterRule: Equatable, Identifiable {
    let id: String
    var srcMac: String
    var srcIP: String
    var srcPort: String
    var destIP: String
    var destPort: String
    var protocol_: String
    var enabled: Bool
}

enum FirewallParser {
    static func parseConfig(_ data: [String: Any]) -> FirewallConfig {
        FirewallConfig(
            enabled: asBool(data["firewall_switch"]),
            nat: asBool(data["nat_switch"]),
            dmzEnabled: asBool(data["dmz_enabled"]),
            dmzHost: data["dmz_ip"] as? String ?? "",
            level: data["firewall_level"] as? String ?? "medium",
            wanPingFilter: asBool(data["wan_ping_filter"]),
            portForwardEnabled: asBool(data["port_forward_switch"])
        )
    }

    static func parsePortForwardRules(_ data: [String: Any]) -> [PortForwardRule] {
        guard let rules = data["rule_list"] as? [[String: Any]] else { return [] }
        return rules.enumerated().map { index, rule in
            PortForwardRule(
                id: rule["id"] as? String ?? "\(index)",
                name: rule["name"] as? String ?? "",
                protocol_: rule["protocol"] as? String ?? "",
                wanPort: rule["wan_port"] as? String ?? "",
                lanIP: rule["lan_ip"] as? String ?? "",
                lanPort: rule["lan_port"] as? String ?? "",
                enabled: asBool(rule["enabled"])
            )
        }
    }

    static func parseFilterRules(_ data: [String: Any]) -> [FilterRule] {
        guard let rules = data["rule_list"] as? [[String: Any]] else { return [] }
        return rules.enumerated().map { index, rule in
            FilterRule(
                id: rule["id"] as? String ?? "\(index)",
                srcMac: rule["src_mac"] as? String ?? "",
                srcIP: rule["src_ip"] as? String ?? "",
                srcPort: rule["src_port"] as? String ?? "",
                destIP: rule["dest_ip"] as? String ?? "",
                destPort: rule["dest_port"] as? String ?? "",
                protocol_: rule["protocol"] as? String ?? "",
                enabled: asBool(rule["enabled"])
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

// MARK: - Telemetry / Domain Filter

struct DomainFilterConfig: Equatable {
    var enabled: Bool
    var rules: [DomainFilterRule]

    static let empty = DomainFilterConfig(enabled: false, rules: [])
}

struct DomainFilterRule: Equatable, Identifiable {
    let id: String
    var domain: String
    var enabled: Bool
}

enum TelemetryParser {
    static func parseDomainFilter(_ data: [String: Any]) -> DomainFilterConfig {
        let enabled = asBool(data["enable"])
        var rules: [DomainFilterRule] = []
        if let ruleList = data["rule_list"] as? [[String: Any]] {
            rules = ruleList.enumerated().map { index, rule in
                DomainFilterRule(
                    id: rule["id"] as? String ?? "\(index)",
                    domain: rule["domain"] as? String ?? "",
                    enabled: asBool(rule["enabled"])
                )
            }
        }
        return DomainFilterConfig(enabled: enabled, rules: rules)
    }

    private static func asBool(_ value: Any?) -> Bool {
        if let str = value as? String {
            return str == "1" || str.lowercased() == "true" || str.lowercased() == "on"
        }
        if let num = value as? Int { return num != 0 }
        if let b = value as? Bool { return b }
        return false
    }

    static let knownTelemetryDomains = [
        "dclient.ztems.com",
        "dconfig.ztems.com",
        "iot.ztems.com",
        "mcs-cloud.ztems.com",
        "update.ztems.com"
    ]
}

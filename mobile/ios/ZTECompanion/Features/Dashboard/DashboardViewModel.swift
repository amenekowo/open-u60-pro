import SwiftUI
import os

private let logger = Logger(subsystem: "com.zte.companion", category: "Dashboard")

@Observable
@MainActor
final class DashboardViewModel {
    var nrSignal: NRSignal = .empty
    var lteSignal: LTESignal = .empty
    var operatorInfo: OperatorInfo = .empty
    var battery: BatteryStatus = .empty
    var thermal: ThermalStatus = .empty
    var speed: TrafficSpeed = .zero
    var trafficStats: TrafficStats = .empty
    var wanIPv4: String = ""
    var wanIPv6: String = ""
    var wifiStatus: WifiStatus = .empty
    var systemInfo: SystemInfo = .empty
    var connectedDevices: [ConnectedDevice] = []
    var isLoading: Bool = false
    var lastUpdated: Date?
    var error: String?

    private let client: UbusClient
    private let authManager: AuthManager
    private var pollTask: Task<Void, Never>?
    private var previousTraffic: TrafficStats?
    private var prevCpuSample: CpuStatSample?
    private var cpuFileReadFailCount: Int = 0
    private var cpuFileReadCooldown: Int = 0
    private var battCurrentFileReadFailCount: Int = 0
    private var battCurrentFileReadCooldown: Int = 0
    private var companionBwFailCount: Int = 0
    private var companionBwCooldown: Int = 0
    private var cachedCpuCores: Int = 4 // SDX75 default
    private static let maxCpuFileReadFails = 3

    private static func isCancellation(_ error: Error) -> Bool {
        if error is CancellationError { return true }
        if let ubusError = error as? UbusError,
           case .networkError(let inner) = ubusError,
           (inner as? URLError)?.code == .cancelled { return true }
        return false
    }

    init(client: UbusClient, authManager: AuthManager) {
        self.client = client
        self.authManager = authManager
    }

    func startPolling(interval: TimeInterval = 2.0) {
        stopPolling()
        pollTask = Task {
            while !Task.isCancelled {
                await refresh()
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }

    func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }

    func refresh() async {
        logger.debug("refresh start")
        var token = authManager.sessionToken
        error = nil

        // Signal fetch first (needs re-auth check)
        var signalResult = await fetchSignal(token: token)

        // Session expired (ubus code 6)? Re-authenticate once and retry.
        if signalResult == nil, await authManager.reauthenticate() {
            token = authManager.sessionToken
            cpuFileReadFailCount = 0
            cpuFileReadCooldown = 0
            battCurrentFileReadFailCount = 0
            battCurrentFileReadCooldown = 0
            companionBwFailCount = 0
            companionBwCooldown = 0
            signalResult = await fetchSignal(token: token)
        }

        // Parallelize remaining independent network calls
        let t = token
        async let batteryResult = fetchBattery(token: t)
        async let chargerResult = fetchCharger(token: t)
        async let thermalResult = fetchThermal(token: t)
        async let trafficResult = fetchTraffic(token: t)
        async let deviceList = fetchDevices(token: t)
        async let wanResult = fetchWAN(token: t)
        async let wan6Result = fetchWAN6(token: t)
        async let wifiResult = fetchWifi(token: t)
        async let cpuResult = fetchSystemInfo(token: t)
        async let cpuUsage = fetchCpuUsage(token: t)
        async let battCurrentResult = fetchBatteryCurrent(token: t)

        let (bat, charger, therm, traffic, devices, wan, wan6, wifi, cpu, cpuUse, battCurrent) = await (
            batteryResult, chargerResult, thermalResult, trafficResult,
            deviceList, wanResult, wan6Result, wifiResult, cpuResult, cpuUsage,
            battCurrentResult
        )

        if let (nr, lte, _, op) = signalResult {
            if nr != nrSignal { nrSignal = nr }
            if lte != lteSignal { lteSignal = lte }
            if op != operatorInfo { operatorInfo = op }
        }
        if var b = bat {
            if let chargerData = charger {
                DeviceParser.parseCharger(chargerData, into: &b)
            }
            b.currentMA = battCurrent.current
            b.voltageMV = battCurrent.voltage
            if b != battery { battery = b }
        }
        if let t = therm, t != thermal { thermal = t }
        if let traffic {
            if let prev = previousTraffic {
                let newSpeed = DeviceParser.computeSpeed(previous: prev, current: traffic)
                if newSpeed != speed { speed = newSpeed }
            }
            previousTraffic = traffic
            if traffic != trafficStats { trafficStats = traffic }
        }
        if let devices, devices != connectedDevices { connectedDevices = devices }
        if let w = wan, w != wanIPv4 { wanIPv4 = w }
        if let w6 = wan6, w6 != wanIPv6 { wanIPv6 = w6 }
        if let wifi, wifi != wifiStatus { wifiStatus = wifi }
        if var cpu {
            if let usage = cpuUse {
                cpu.cpuUsagePercent = usage
                cpu.cpuUsageIsEstimate = false
            }
            cpu.cpuCores = cachedCpuCores
            if cpu != systemInfo { systemInfo = cpu }
        }

        lastUpdated = Date()
        logger.debug("refresh done")
    }

    private func fetchSignal(token: String) async -> (NRSignal, LTESignal, WCDMASignal, OperatorInfo)? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zte_nwinfo_api",
                method: "nwinfo_get_netinfo", params: [:]
            )
            return SignalParser.parseNetInfo(data)
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    private func fetchBattery(token: String) async -> BatteryStatus? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zwrt_bsp.battery",
                method: "list", params: [:]
            )
            return DeviceParser.parseBattery(data)
        } catch { return nil }
    }

    private func fetchCharger(token: String) async -> [String: Any]? {
        guard let (_, data) = try? await client.call(
            sessionToken: token, object: "zwrt_bsp.charger",
            method: "list", params: [:]
        ) else { return nil }
        return data
    }

    private func fetchThermal(token: String) async -> ThermalStatus? {
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zwrt_bsp.thermal",
                method: "get_cpu_temp", params: [:]
            )
            return DeviceParser.parseThermal(data)
        } catch { return nil }
    }

    private func fetchTraffic(token: String) async -> TrafficStats? {
        // Priority 1: zte-companion.bandwidth (kernel-level /proc/net/dev)
        if let stats = await fetchCompanionBandwidth(token: token) {
            return stats
        }
        // Priority 2: zwrt_data get_wwandst (modem pre-computed rates)
        if let (_, data) = try? await client.call(
            sessionToken: token, object: "zwrt_data",
            method: "get_wwandst", params: [:]
        ), let stats = DeviceParser.parseWwandstTraffic(data) {
            return stats
        }
        // Priority 3: network.device status (rmnet_data0 delta)
        if let (_, data) = try? await client.call(
            sessionToken: token, object: "network.device",
            method: "status", params: ["name": "rmnet_data0"]
        ) {
            var stats = DeviceParser.parseTraffic(data)
            stats.source = "rmnet_ubus"
            return stats
        }
        return nil
    }

    private func fetchCompanionBandwidth(token: String) async -> TrafficStats? {
        if companionBwCooldown > 0 {
            companionBwCooldown -= 1
            return nil
        }
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zte-companion",
                method: "bandwidth", params: [:]
            )
            guard let interfaces = data["if"] as? [String: Any],
                  let rmnet = interfaces["rmnet_data0"] as? [String: Any],
                  let rx = (rmnet["rx"] as? UInt64) ?? (rmnet["rx"] as? Int).map({ UInt64($0) }),
                  let tx = (rmnet["tx"] as? UInt64) ?? (rmnet["tx"] as? Int).map({ UInt64($0) }) else {
                return nil
            }
            companionBwFailCount = 0
            companionBwCooldown = 0
            return TrafficStats(rxBytes: rx, txBytes: tx, timestamp: Date(), source: "companion")
        } catch {
            if Self.isCancellation(error) { return nil }
            companionBwFailCount += 1
            logger.warning("companion bandwidth error: \(String(describing: error)) (fail \(self.companionBwFailCount)/\(Self.maxCpuFileReadFails))")
            if companionBwFailCount >= Self.maxCpuFileReadFails {
                companionBwCooldown = 10
                logger.warning("companion bandwidth cooldown (retry in ~10 cycles)")
            }
            return nil
        }
    }

    private func fetchDevices(token: String) async -> [ConnectedDevice]? {
        do {
            let (_, hintsData) = try await client.call(
                sessionToken: token, object: "luci-rpc",
                method: "getHostHints", params: [:]
            )
            var deviceList = DeviceParser.parseHostHints(hintsData)

            // Enrich with DHCP hostnames (optional)
            if let (_, dhcpData) = try? await client.call(
                sessionToken: token, object: "luci-rpc",
                method: "getDHCPLeases", params: ["family": 4]
            ), let leases = dhcpData["dhcp_leases"] as? [[String: Any]] {
                DeviceParser.enrichWithDHCP(devices: &deviceList, leases: leases)
            }

            return deviceList
        } catch { return nil }
    }

    private func fetchWAN(token: String) async -> String? {
        guard let (_, data) = try? await client.call(
            sessionToken: token, object: "network.interface.zte_wan",
            method: "status", params: [:]
        ) else { return nil }
        let ip = DeviceParser.parseWanIPv4(data)
        return ip.isEmpty ? nil : ip
    }

    private func fetchWAN6(token: String) async -> String? {
        guard let (_, data) = try? await client.call(
            sessionToken: token, object: "network.interface.zte_wan6",
            method: "status", params: [:]
        ) else { return nil }
        let ip = DeviceParser.parseWanIPv6(data)
        return ip.isEmpty ? nil : ip
    }

    private func fetchWifi(token: String) async -> WifiStatus? {
        guard let (_, statusData) = try? await client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "report", params: [:]
        ) else { return nil }
        var wifi = DeviceParser.parseWifiStatus(statusData)

        // Fire 6 independent calls in parallel (same pattern as refresh())
        async let chanCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "get_current_channel", params: [:]
        )
        async let ifaceCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "iface_report", params: [:]
        )
        async let tx2gCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "wlan_uci_get_section", params: ["section": "wifi0"]
        )
        async let tx5gCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "wlan_uci_get_section", params: ["section": "wifi1"]
        )
        async let mbbCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "wlan_uci_get_section", params: ["section": "zte_mbb"]
        )
        async let assocCall = client.call(
            sessionToken: token, object: "zwrt_wlan",
            method: "get_assoc_info", params: [:]
        )

        let chan = try? await chanCall
        let iface = try? await ifaceCall
        let tx2g = try? await tx2gCall
        let tx5g = try? await tx5gCall
        let mbb = try? await mbbCall
        let assoc = try? await assocCall

        if let (_, d) = chan { DeviceParser.parseWifiChannels(d, into: &wifi) }
        if let (_, d) = iface { DeviceParser.parseWifiInterfaces(d, into: &wifi) }
        if let (_, d) = tx2g { DeviceParser.parseWifiTxPower(d, band: "2g", into: &wifi) }
        if let (_, d) = tx5g { DeviceParser.parseWifiTxPower(d, band: "5g", into: &wifi) }
        if let (_, d) = mbb { DeviceParser.parseWifi6(d, into: &wifi) }
        if let (_, d) = assoc { DeviceParser.parseWifiClients(d, into: &wifi) }

        return wifi
    }

    private func fetchSystemInfo(token: String) async -> SystemInfo? {
        guard let (_, data) = try? await client.call(
            sessionToken: token, object: "system",
            method: "info", params: [:]
        ) else { return nil }
        return DeviceParser.parseSystemInfo(data, cpuCores: cachedCpuCores)
    }

    private func fetchBatteryCurrent(token: String) async -> (current: Int?, voltage: Int?) {
        if battCurrentFileReadCooldown > 0 {
            battCurrentFileReadCooldown -= 1
            return (nil, nil)
        }
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zte-companion",
                method: "battery_current", params: [:]
            )
            guard let microamps = data["current_now"] as? Int else {
                logger.debug("battery current_now: missing or unparseable data")
                return (nil, nil)
            }
            battCurrentFileReadFailCount = 0
            battCurrentFileReadCooldown = 0
            let voltageMV: Int? = (data["voltage_now"] as? Int).map { $0 / 1000 }
            return (microamps / 1000, voltageMV)
        } catch let error as UbusError {
            if Self.isCancellation(error) { return (nil, nil) }
            battCurrentFileReadFailCount += 1
            logger.warning("battery current_now error: \(String(describing: error)) (fail \(self.battCurrentFileReadFailCount)/\(Self.maxCpuFileReadFails))")
            if case .requestFailed = error, battCurrentFileReadFailCount >= Self.maxCpuFileReadFails {
                battCurrentFileReadCooldown = 10
                logger.warning("battery current_now cooldown after \(Self.maxCpuFileReadFails) consecutive failures (retry in ~10 cycles)")
            }
            return (nil, nil)
        } catch {
            if Self.isCancellation(error) { return (nil, nil) }
            battCurrentFileReadFailCount += 1
            logger.warning("battery current_now unexpected error: \(String(describing: error))")
            return (nil, nil)
        }
    }

    private func fetchCpuUsage(token: String) async -> Double? {
        if cpuFileReadCooldown > 0 {
            cpuFileReadCooldown -= 1
            return nil
        }
        do {
            let (_, data) = try await client.call(
                sessionToken: token, object: "zte-companion",
                method: "cpu_usage", params: [:]
            )
            // Response: {idle: <u64>, total: <u64>, cores: <int>}
            // Handle Swift JSON number coercion: values may arrive as Int or UInt64
            guard let idle = (data["idle"] as? UInt64) ?? (data["idle"] as? Int).map({ UInt64($0) }),
                  let total = (data["total"] as? UInt64) ?? (data["total"] as? Int).map({ UInt64($0) }) else {
                logger.debug("cpu_usage: missing or unparseable idle/total")
                return nil
            }
            if let cores = (data["cores"] as? Int) ?? (data["cores"] as? UInt64).map({ Int($0) }) {
                cachedCpuCores = cores
            }
            let sample = CpuStatSample(idle: idle, total: total)
            cpuFileReadFailCount = 0
            cpuFileReadCooldown = 0
            defer { prevCpuSample = sample }
            guard let prev = prevCpuSample else {
                logger.debug("cpu_usage: first sample collected (\(self.cachedCpuCores) cores)")
                return nil
            }
            let usage = DeviceParser.computeCpuUsage(previous: prev, current: sample)
            logger.debug("cpu_usage: usage=\(usage.map { String(format: "%.1f%%", $0) } ?? "nil")")
            return usage
        } catch let error as UbusError {
            if Self.isCancellation(error) { return nil }
            cpuFileReadFailCount += 1
            logger.warning("cpu_usage error: \(String(describing: error)) (fail \(self.cpuFileReadFailCount)/\(Self.maxCpuFileReadFails))")
            if case .requestFailed = error, cpuFileReadFailCount >= Self.maxCpuFileReadFails {
                cpuFileReadCooldown = 10
                logger.warning("cpu_usage cooldown after \(Self.maxCpuFileReadFails) consecutive failures (retry in ~10 cycles)")
            }
            return nil
        } catch {
            if Self.isCancellation(error) { return nil }
            cpuFileReadFailCount += 1
            logger.warning("cpu_usage unexpected error: \(String(describing: error))")
            return nil
        }
    }
}

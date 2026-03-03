import SwiftUI

struct DeviceInfoView: View {
    var viewModel: DeviceInfoViewModel

    var body: some View {
        NavigationStack {
            List {
                Section("SIM Card") {
                    infoRow("ICCID", viewModel.identity.simICCID)
                    infoRow("IMSI", viewModel.identity.simIMSI)
                    infoRow("MSISDN", viewModel.identity.msisdn)
                    infoRow("SPN", viewModel.identity.spn)
                    mccMncRow
                    simStatusRow
                }

                Section("Device") {
                    infoRow("IMEI", viewModel.identity.imei)
                }

                Section("Network") {
                    roamingRow
                    infoRow("APN", viewModel.identity.wanAPN)
                    signalBarsRow
                }

                Section("WAN") {
                    infoRow("IPv4", viewModel.identity.wanIPv4)
                    ForEach(viewModel.identity.wanIPv6, id: \.self) { addr in
                        infoRow("IPv6", addr)
                    }
                    if viewModel.identity.wanIPv6.isEmpty {
                        infoRow("IPv6", "")
                    }
                }

                Section("LAN") {
                    infoRow("Gateway", viewModel.identity.lanIP)
                }
            }
            .navigationTitle("Device Info")
            .refreshable { await viewModel.refresh() }
            .overlay {
                if viewModel.isLoading {
                    ProgressView()
                }
            }
            .task { await viewModel.refresh() }
        }
    }

    // MARK: - Rows

    private func infoRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value.isEmpty ? "--" : value)
                .font(.body.monospacedDigit())
                .textSelection(.enabled)
        }
    }

    private var mccMncRow: some View {
        let mcc = viewModel.identity.mcc
        let mnc = viewModel.identity.mnc
        let value = (mcc.isEmpty && mnc.isEmpty) ? "" : "\(mcc)/\(mnc)"
        return infoRow("MCC/MNC", value)
    }

    private var simStatusRow: some View {
        let raw = viewModel.identity.simStatus
        let label = simStatusLabel(raw)
        let color = simStatusColor(raw)
        return HStack {
            Text("SIM Status")
                .foregroundStyle(.secondary)
            Spacer()
            Text(label)
                .font(.body.monospacedDigit())
                .foregroundStyle(color)
        }
    }

    private var roamingRow: some View {
        let roaming = viewModel.operatorInfo.roaming
        return HStack {
            Text("Roaming")
                .foregroundStyle(.secondary)
            Spacer()
            Text(roaming ? "Roaming" : "Home")
                .font(.body.monospacedDigit())
                .foregroundStyle(roaming ? .orange : .green)
        }
    }

    private var signalBarsRow: some View {
        let bars = viewModel.operatorInfo.signalBar
        let maxBars = 5
        return HStack {
            Text("Signal")
                .foregroundStyle(.secondary)
            Spacer()
            HStack(spacing: 2) {
                ForEach(0..<maxBars, id: \.self) { i in
                    RoundedRectangle(cornerRadius: 1)
                        .fill(i < bars ? signalColor(bars: bars, max: maxBars) : Color.gray.opacity(0.3))
                        .frame(width: 4, height: CGFloat(6 + i * 3))
                }
            }
            Text("\(bars)/\(maxBars)")
                .font(.body.monospacedDigit())
                .foregroundStyle(.secondary)
                .padding(.leading, 4)
        }
    }

    // MARK: - Helpers

    private func simStatusLabel(_ raw: String) -> String {
        switch raw.lowercased() {
        case "", "unknown": return "--"
        case "ready", "sim_ready": return "Ready"
        case "not_inserted", "no_sim", "sim_absent": return "No SIM"
        case "pin_required", "sim_pin": return "PIN Required"
        case "puk_required", "sim_puk": return "PUK Required"
        case "error", "sim_error": return "Error"
        default: return raw
        }
    }

    private func simStatusColor(_ raw: String) -> Color {
        switch raw.lowercased() {
        case "ready", "sim_ready": return .green
        case "not_inserted", "no_sim", "sim_absent": return .red
        case "pin_required", "sim_pin", "puk_required", "sim_puk": return .orange
        case "error", "sim_error": return .red
        default: return .secondary
        }
    }

    private func signalColor(bars: Int, max: Int) -> Color {
        let ratio = Double(bars) / Double(max)
        if ratio >= 0.6 { return .green }
        if ratio >= 0.4 { return .yellow }
        return .red
    }
}

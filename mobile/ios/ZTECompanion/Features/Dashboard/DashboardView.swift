import SwiftUI

struct DashboardView: View {
    var viewModel: DashboardViewModel
    let isAuthenticated: Bool
    let client: UbusClient
    let authManager: AuthManager

    @State private var signalMonitorVM: SignalMonitorViewModel
    @State private var showAllDevices = true

    init(viewModel: DashboardViewModel, isAuthenticated: Bool, client: UbusClient, authManager: AuthManager) {
        self.viewModel = viewModel
        self.isAuthenticated = isAuthenticated
        self.client = client
        self.authManager = authManager
        _signalMonitorVM = State(initialValue: SignalMonitorViewModel(client: client, authManager: authManager))
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    if viewModel.simPukRequired {
                        SIMAlertBanner(
                            icon: "exclamationmark.lock.fill",
                            title: "SIM PUK Required",
                            message: "Too many wrong PIN attempts. Go to Router > SIM Card to enter your PUK.",
                            color: .red
                        )
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                    if viewModel.simPinRequired {
                        SIMAlertBanner(
                            icon: "lock.fill",
                            title: "SIM PIN Required",
                            message: "Your SIM card is locked. Go to Router > SIM Card to enter your PIN.",
                            color: .orange
                        )
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                    if viewModel.isAirplaneMode {
                        SIMAlertBanner(
                            icon: "airplane",
                            title: "Airplane Mode",
                            message: "Cellular radio is off. The modem is powered down — no signal or data.",
                            color: .blue
                        )
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                    if viewModel.isMobileDataOff && !viewModel.isAirplaneMode {
                        SIMAlertBanner(
                            icon: "antenna.radiowaves.left.and.right.slash",
                            title: "Mobile Data Off",
                            message: "Cellular radio is on but data is disabled. Go to Router > Mobile Network to enable it.",
                            color: .orange
                        )
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                    if viewModel.companionUnavailable {
                        SIMAlertBanner(
                            icon: "puzzlepiece.extension",
                            title: "Companion Plugin Unavailable",
                            message: "CPU, battery current, and bandwidth data are degraded. Run \"zte companion repair\" to fix.",
                            color: .purple
                        )
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                    OperatorCardView(
                        operatorInfo: viewModel.operatorInfo,
                        nrSignal: viewModel.nrSignal,
                        lteSignal: viewModel.lteSignal
                    )
                    NavigationLink {
                        SignalMonitorView(viewModel: signalMonitorVM)
                    } label: {
                        SignalCardView(
                            operatorInfo: viewModel.operatorInfo,
                            nrSignal: viewModel.nrSignal,
                            lteSignal: viewModel.lteSignal,
                            isAirplaneMode: viewModel.isAirplaneMode
                        )
                    }
                    .buttonStyle(.plain)
                    CellularCardView(
                        wanIPv4: viewModel.wanIPv4,
                        wanIPv6: viewModel.wanIPv6,
                        speed: viewModel.speed,
                        trafficStats: viewModel.trafficStats
                    )
                    HStack(spacing: 16) {
                        BatteryCardView(battery: viewModel.battery)
                        CPUCardView(systemInfo: viewModel.systemInfo, thermal: viewModel.thermal)
                    }
                    WiFiCardView(wifiStatus: viewModel.wifiStatus)
                    DevicesCardView(
                        connectedDevices: viewModel.connectedDevices,
                        showAllDevices: $showAllDevices
                    )
                }
                .padding()
            }
            .background(Color(.systemGroupedBackground))
            .navigationTitle("Dashboard")
            .refreshable { await viewModel.refresh() }
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    LastUpdatedView(date: viewModel.lastUpdated)
                }
                ToolbarItem(placement: .topBarLeading) {
                    connectionIndicator
                }
            }
        }
    }

    private var connectionIndicator: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(isAuthenticated ? .green : .red)
                .frame(width: 10, height: 10)
            Text(isAuthenticated ? "Connected" : "Offline")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(isAuthenticated ? .green : .red)
        }
        .fixedSize()
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule()
                .fill((isAuthenticated ? Color.green : Color.red).opacity(0.12))
        )
    }
}

private struct SIMAlertBanner: View {
    let icon: String
    let title: String
    let message: String
    let color: Color

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundStyle(color)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.subheadline.bold())
                Text(message).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
        }
        .accessibilityElement(children: .combine)
        .padding(12)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(color.opacity(0.3), lineWidth: 1))
    }
}

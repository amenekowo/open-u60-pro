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
                            lteSignal: viewModel.lteSignal
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

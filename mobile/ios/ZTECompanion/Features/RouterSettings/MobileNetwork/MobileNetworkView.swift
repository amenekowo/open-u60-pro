import SwiftUI

struct MobileNetworkView: View {
    @Bindable var viewModel: MobileNetworkViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section("Connection Mode") {
                Picker("Mode", selection: $viewModel.selectedConnectMode) {
                    Text("Automatic").tag(1)
                    Text("Manual").tag(0)
                }
                .pickerStyle(.segmented)
            }

            Section {
                Toggle("Data Roaming", isOn: $viewModel.selectedRoaming)
            } footer: {
                Text("Enabling roaming may incur additional charges from your carrier.")
            }

            Section("Network Selection") {
                Picker("Mode", selection: $viewModel.selectedNetSelectMode) {
                    Text("Automatic").tag("auto_select")
                    Text("Manual").tag("manual_select")
                }
                .pickerStyle(.segmented)

                if viewModel.selectedNetSelectMode == "manual_select" {
                    Button {
                        Task { await viewModel.scanNetworks() }
                    } label: {
                        HStack {
                            Text("Scan Networks")
                            Spacer()
                            if viewModel.isScanning {
                                ProgressView()
                            }
                        }
                    }
                    .disabled(viewModel.isScanning)

                    ForEach(viewModel.config.operators) { op in
                        Button {
                            Task { await viewModel.registerNetwork(mccMnc: op.mccMnc, rat: op.rat) }
                        } label: {
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(op.name)
                                        .foregroundStyle(.primary)
                                    Text("\(op.mccMnc) · \(op.rat)")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                if op.status == "current" {
                                    Image(systemName: "checkmark.circle.fill")
                                        .foregroundStyle(.green)
                                } else if op.status == "forbidden" {
                                    Image(systemName: "xmark.circle")
                                        .foregroundStyle(.red)
                                }
                            }
                        }
                        .disabled(viewModel.isLoading || op.status == "forbidden")
                    }
                }
            }

            Section("APN") {
                LabeledContent("Current APN", value: viewModel.config.currentAPN.isEmpty ? "—" : viewModel.config.currentAPN)

                NavigationLink {
                    APNView(viewModel: APNViewModel(client: viewModel.client, authManager: viewModel.authManager))
                } label: {
                    Text("Manage APN Settings")
                }
            }

            Section {
                Button {
                    Task { await viewModel.applySettings() }
                } label: {
                    Text("Apply")
                        .frame(maxWidth: .infinity)
                }
                .disabled(viewModel.isLoading || !viewModel.hasChanges)
            }
        }
        .navigationTitle("Mobile Network")
        .refreshable { await viewModel.refresh() }
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .task { await viewModel.refresh() }
    }
}

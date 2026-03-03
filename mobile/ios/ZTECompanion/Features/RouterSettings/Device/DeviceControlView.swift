import SwiftUI

struct DeviceControlView: View {
    @Bindable var viewModel: DeviceControlViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section {
                Toggle("Power Supply", isOn: $viewModel.powerSupplyEnabled)
                    .disabled(viewModel.isLoading)
                    .onChange(of: viewModel.powerSupplyEnabled) { _, newValue in
                        Task { await viewModel.setPowerSupply(enabled: newValue) }
                    }
            } footer: {
                Text("When enabled, the device runs directly from the AC adapter and maintains battery at 40–60% to extend battery lifespan.")
            }

            Section {
                Toggle("Power-save Mode", isOn: $viewModel.powerSaveEnabled)
                    .disabled(viewModel.isLoading)
                    .onChange(of: viewModel.powerSaveEnabled) { _, newValue in
                        Task { await viewModel.setPowerSave(enabled: newValue) }
                    }
            } footer: {
                Text("Restricts data communication speed to reduce consumption and extend battery life.")
            }

            Section {
                Button("Reboot Router") {
                    viewModel.showRebootConfirm = true
                }
                .disabled(viewModel.isLoading)
            } footer: {
                Text("The router will restart. This takes about 60 seconds.")
            }

            Section {
                Button("Factory Reset", role: .destructive) {
                    viewModel.showFactoryResetConfirm = true
                }
                .disabled(viewModel.isLoading)
            } footer: {
                Text("This will erase all settings and restore factory defaults. This cannot be undone.")
            }
        }
        .task { await viewModel.refresh() }
        .navigationTitle("Device Controls")
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .sheet(isPresented: $viewModel.showRebootConfirm) {
            PasswordConfirmView(
                title: "Reboot Router",
                message: "Enter your router password to confirm reboot.",
                confirmLabel: "Reboot"
            ) {
                await viewModel.reboot()
            }
        }
        .sheet(isPresented: $viewModel.showFactoryResetConfirm) {
            PasswordConfirmView(
                title: "Factory Reset",
                message: "This will erase ALL settings. Enter your router password to confirm.",
                confirmLabel: "Factory Reset"
            ) {
                await viewModel.factoryReset()
            }
        }
    }
}

import SwiftUI
import UniformTypeIdentifiers

struct RootView: View {
    @EnvironmentObject var model: StoreModel
    @Environment(\.scenePhase) var phase
    var body: some View {
        TabView {
            LibraryView().tabItem { Label("Aplikacje", systemImage: "square.stack.3d.up") }
            HistoryView().tabItem { Label("Historia", systemImage: "clock") }
            ConnectionView().tabItem { Label("Połączenie", systemImage: "network") }
        }
        .task(id: phase) {
            guard phase == .active else { return }
            while !Task.isCancelled {
                await model.refresh()
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }
}

struct ServerNotice: View {
    @EnvironmentObject var model: StoreModel
    var body: some View {
        if let error = model.error { Label(error, systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(.orange).padding(.vertical, 6) }
        if model.working { HStack { ProgressView(); Text("Operacja w toku…").font(.footnote) } }
    }
}

struct LibraryView: View {
    @EnvironmentObject var model: StoreModel
    @State private var importing = false
    @State private var selected: Artifact?
    var body: some View {
        NavigationStack {
            List {
                Section { ServerNotice() }
                if let state = model.snapshot {
                    Section {
                        Label(state.deviceName, systemImage: "iphone").foregroundStyle(Color.mostGreen)
                        Text(state.appleReady ? "Konto Apple gotowe na PC" : "Podpisywanie: zaloguj konto Apple na PC").font(.caption).foregroundStyle(.secondary)
                    }
                    Section("Biblioteka na Twoim PC") {
                        if state.artifacts.isEmpty { Text("Dodaj IPA z Plików albo z panelu MostServer.").foregroundStyle(.secondary) }
                        ForEach(state.artifacts) { artifact in
                            Button { selected = artifact } label: {
                                HStack(spacing: 14) {
                                    Image(systemName: "app.dashed").font(.largeTitle).foregroundStyle(Color.mostGreen)
                                    VStack(alignment: .leading, spacing: 5) {
                                        Text(artifact.metadata.name).foregroundStyle(.primary).font(.headline)
                                        Text("v\(artifact.metadata.version) · build \(artifact.metadata.build)").font(.caption).foregroundStyle(.secondary)
                                        Text(artifact.metadata.bundleId).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    Spacer()
                                    Image(systemName: "arrow.down.circle").foregroundStyle(Color.mostGreen)
                                }.padding(.vertical, 7)
                            }
                        }
                    }
                    Section { Text("Instalację prowadzi MostServer przez połączenie PC–iPhone USB/Wi-Fi. Zdalny relay LocalDevVPN nie jest jeszcze częścią tego wydania.").font(.footnote).foregroundStyle(.secondary) }
                } else {
                    ContentUnavailableView("Połącz swój komputer", systemImage: "desktopcomputer", description: Text("Na ekranie Połączenie zeskanuj kod QR z MostServer. PC i iPhone muszą być w tej samej sieci."))
                }
            }
            .navigationTitle("Twoje aplikacje.")
            .toolbar { ToolbarItem(placement: .topBarTrailing) { Button { importing = true } label: { Label("Importuj IPA", systemImage: "plus") }.disabled(model.snapshot == nil || model.working) } }
            .refreshable { await model.refresh() }
            .fileImporter(isPresented: $importing, allowedContentTypes: [UTType(filenameExtension: "ipa") ?? .zip], allowsMultipleSelection: false) { result in
                switch result { case .success(let files): if let file = files.first { Task { await model.importFile(file) } }; case .failure(let error): model.error = error.localizedDescription }
            }
            .sheet(item: $selected) { artifact in
                NavigationStack {
                    List {
                        Section { Text(artifact.metadata.name).font(.title2.bold()); Text(artifact.metadata.bundleId).font(.caption).foregroundStyle(.secondary) }
                        Section("Instalacja") {
                            Button("Podpisz kontem Apple i zainstaluj") { selected = nil; Task { await model.install(artifact, mode: "resign") } }
                                .disabled(model.snapshot?.appleReady != true || model.snapshot?.busy == true || model.working || artifact.metadata.extensions > 0)
                            Button("Zainstaluj wcześniej podpisaną IPA") { selected = nil; Task { await model.install(artifact, mode: "presigned") } }
                                .disabled(!artifact.metadata.hasSignatureFiles || artifact.metadata.profile == nil || model.snapshot?.busy == true || model.working)
                        }
                        Section { Text("iOS sprawdza podpis i profil. Jeśli wybierzesz podpisanie, operacja wykorzysta zespół wybrany na PC i mapowanie identyfikatora silnika isideload. Ta wersja podpisuje podstawowe aplikacje bez rozszerzeń.").font(.footnote).foregroundStyle(.secondary) }
                    }.navigationTitle("Instaluj IPA").toolbar { ToolbarItem(placement: .cancellationAction) { Button("Zamknij") { selected = nil } } }
                }.presentationDetents([.medium, .large])
            }
        }
    }
}

struct HistoryView: View {
    @EnvironmentObject var model: StoreModel
    var body: some View {
        NavigationStack {
            List {
                Section { ServerNotice() }
                if let state = model.snapshot, !state.jobs.isEmpty {
                    ForEach(state.jobs) { job in
                        VStack(alignment: .leading, spacing: 10) {
                            Label(job.label, systemImage: job.state == "succeeded" ? "checkmark.circle" : "arrow.triangle.2.circlepath").foregroundStyle(job.state == "succeeded" ? Color.mostGreen : .primary).font(.headline)
                            if let artifact = state.artifacts.first(where: { $0.id == job.artifactId }) { Text(artifact.metadata.name).font(.subheadline) }
                            Text(job.message).font(.footnote).foregroundStyle(.secondary)
                            if job.state == "installing", let progress = job.progress { ProgressView(value: Double(progress), total: 100) }
                            if job.state == "needs_reconciliation", job.effectiveBundleId != nil {
                                Button("Sprawdź wynik na urządzeniu") { Task { await model.reconcile(job) } }.disabled(model.working || state.busy)
                            }
                        }.padding(.vertical, 8)
                    }
                } else { ContentUnavailableView("Brak zadań", systemImage: "clock", description: Text("Tutaj zobaczysz rzeczywiste wyniki instalacji z Twojego PC.")) }
            }.navigationTitle("Historia").refreshable { await model.refresh() }
        }
    }
}

struct ConnectionView: View {
    @EnvironmentObject var model: StoreModel
    @State private var scanning = false
    @State private var scanValue: String?
    @State private var text = ""
    @State private var candidate: Pairing?
    @State private var confirm = false
    @State private var disconnect = false
    var body: some View {
        NavigationStack {
            Form {
                Section { ServerNotice() }
                Section("Twój komputer") {
                    if let address = model.address {
                        Label(address, systemImage: "lock.shield").font(.footnote)
                        Text("Certyfikat PC jest przypięty. Klucz dostępu przechowuje Keychain tego urządzenia.").font(.caption).foregroundStyle(.secondary)
                        Button("Odśwież połączenie") { Task { await model.refresh() } }
                        Button("Usuń połączenie z iPhone’a", role: .destructive) { disconnect = true }
                    } else {
                        Text("Na PC otwórz Połączenie, włącz MostStore LAN, a na ekranie Urządzenia wygeneruj kod QR dla tego telefonu.")
                    }
                    Button { scanning = true } label: { Label("Skanuj kod z MostServer", systemImage: "qrcode.viewfinder") }.disabled(model.working)
                }
                Section("Lub wklej konfigurację QR") {
                    TextEditor(text: $text).frame(minHeight: 90).font(.caption.monospaced()).autocorrectionDisabled().textInputAutocapitalization(.never).privacySensitive()
                    Button("Sprawdź konfigurację") { parse(text); text = "" }.disabled(text.isEmpty || model.working)
                }
                Section("Zakres wersji preview") {
                    Label("MostStore LAN · HTTPS", systemImage: "network")
                    Text("PC instaluje aplikacje przez własne połączenie USB/Wi-Fi z iPhonem. Musi pozostać włączony.").font(.footnote)
                    Label("Remote + LocalDevVPN: do implementacji", systemImage: "wrench.and.screwdriver")
                    Text("Ta aplikacja nie zawiera providera VPN, JIT ani zdalnego mostu usług urządzenia.").font(.footnote).foregroundStyle(.secondary)
                }
            }.navigationTitle("Połączenie")
            .sheet(isPresented: $scanning, onDismiss: { if let value = scanValue { scanValue = nil; parse(value) } }) { QRScanner { result in switch result { case .success(let value): scanValue = value; case .failure(let error): model.error = error.localizedDescription }; scanning = false }.ignoresSafeArea().overlay(alignment: .topTrailing) { Button("Zamknij") { scanning = false }.padding().background(.ultraThinMaterial).clipShape(Capsule()).padding() } }
            .alert("Zaufać temu komputerowi?", isPresented: $confirm) { Button("Połącz") { if let pairing = candidate { Task { await model.connect(pairing) } }; candidate = nil }; Button("Anuluj", role: .cancel) { candidate = nil } } message: { Text(candidate?.url ?? "") }
            .confirmationDialog("Usunąć lokalnie zapisane połączenie?", isPresented: $disconnect, titleVisibility: .visible) { Button("Usuń połączenie", role: .destructive) { model.disconnect() }; Button("Anuluj", role: .cancel) {} } message: { Text("Zaufanie możesz dodatkowo odwołać na PC w ekranie Połączenie.") }
        }
    }
    private func parse(_ value: String) {
        guard value.utf8.count <= 4096 else { model.error = "Konfiguracja QR jest zbyt długa."; return }
        do { let pairing = try JSONDecoder().decode(Pairing.self, from: Data(value.utf8)); try pairing.validate(); candidate = pairing; confirm = true }
        catch { model.error = error.localizedDescription }
    }
}

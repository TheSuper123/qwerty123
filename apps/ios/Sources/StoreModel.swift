import SwiftUI

@MainActor
final class StoreModel: ObservableObject {
    @Published var snapshot: StoreSnapshot?
    @Published var error: String?
    @Published var working = false
    @Published var address: String?
    private var client: ServerClient?
    private var refreshing = false
    private var requestKeys: [String: String] = [:]

    init() {
        if let pairing = PairingKeychain.load(), let restored = try? ServerClient(pairing: pairing) { client = restored; address = pairing.url }
    }
    func connect(_ pairing: Pairing) async {
        working = true; defer { working = false }
        do {
            let candidate = try ServerClient(pairing: pairing)
            let state = try await candidate.state()
            try PairingKeychain.save(pairing)
            client = candidate; address = pairing.url; snapshot = state; error = nil
        } catch { self.error = error.localizedDescription }
    }
    func disconnect() { client = nil; address = nil; snapshot = nil; error = nil; requestKeys.removeAll(); PairingKeychain.remove() }
    func refresh() async {
        guard let client, !refreshing else { return }
        refreshing = true; defer { refreshing = false }
        do { let state = try await client.state(); guard self.client === client else { return }; snapshot = state; error = nil }
        catch { if self.client === client { self.error = "Nie można odświeżyć stanu PC. \(error.localizedDescription)" } }
    }
    func install(_ artifact: Artifact, mode: String) async {
        guard let client, !working else { return }
        working = true; defer { working = false }
        let identity = artifact.id + ":" + mode
        let key = requestKeys[identity] ?? UUID().uuidString
        requestKeys[identity] = key
        do { try await client.install(artifact, mode: mode, key: key); requestKeys.removeValue(forKey: identity); await refresh() }
        catch { self.error = "\(error.localizedDescription) Sprawdź historię przed ponowieniem; identyfikator zlecenia zostaje zachowany." }
    }
    func reconcile(_ job: InstallJob) async {
        guard let client, !working else { return }
        working = true; defer { working = false }
        do { try await client.reconcile(job); await refresh() } catch { self.error = error.localizedDescription }
    }
    func importFile(_ file: URL) async {
        guard let client, !working else { return }
        working = true; defer { working = false }
        let scoped = file.startAccessingSecurityScopedResource()
        defer { if scoped { file.stopAccessingSecurityScopedResource() } }
        do { try await client.upload(file); await refresh() } catch { self.error = error.localizedDescription }
    }
}

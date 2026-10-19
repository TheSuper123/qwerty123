import Foundation

struct Pairing: Codable {
    let schema: Int
    let url: String
    let token: String
    let certificateSHA256: String
    let deviceID: String
    enum CodingKeys: String, CodingKey { case schema, url, token; case certificateSHA256 = "certificate_sha256"; case deviceID = "device_id" }

    func validate() throws {
        guard schema == 1,
              let parsed = URLComponents(string: url), parsed.scheme == "https",
              parsed.user == nil, parsed.password == nil, parsed.query == nil, parsed.fragment == nil,
              parsed.path.isEmpty || parsed.path == "/", let host = parsed.host,
              token.count == 64, token.allSatisfy({ $0.isHexDigit }),
              certificateSHA256.count == 64, certificateSHA256.allSatisfy({ $0.isHexDigit }),
              !deviceID.isEmpty, deviceID.count <= 128 else { throw StoreError.message("Nieprawidłowy kod połączenia MostServer.") }
        let components = host.split(separator: ".", omittingEmptySubsequences: false)
        let parts = components.compactMap { UInt8($0) }
        guard components.count == 4, parts.count == 4,
              parts[0] == 10 || parts[0] == 127 || (parts[0] == 192 && parts[1] == 168) || (parts[0] == 172 && (16...31).contains(parts[1])) else {
            throw StoreError.message("Ta wersja MostStore łączy się wyłącznie z prywatnym adresem IPv4 PC w Twojej sieci.")
        }
    }
}

struct StoreSnapshot: Decodable {
    let version: String
    let deviceId: String
    let deviceName: String
    let appleReady: Bool
    let busy: Bool
    let artifacts: [Artifact]
    let jobs: [InstallJob]
    let remoteRelay: Bool
    let localVpn: Bool
}
struct Artifact: Decodable, Identifiable {
    let id: String
    let sha256: String
    let size: UInt64
    let metadata: IpaMetadata
}
struct IpaMetadata: Decodable {
    let name: String
    let bundleId: String
    let version: String
    let build: String
    let minimumIos: String?
    let extensions: Int
    let hasSignatureFiles: Bool
    let profile: ProfileSummary?
}
struct ProfileSummary: Decodable { let name: String; let expiresAt: String?; let expired: Bool; let metadataOnly: Bool }
struct InstallJob: Decodable, Identifiable {
    let id: String
    let artifactId: String
    let state: String
    let progress: UInt64?
    let message: String
    let effectiveBundleId: String?
    let updatedAt: String
    var label: String {
        switch state {
        case "succeeded": return "Instalacja potwierdzona"
        case "failed": return "Operacja nieudana"
        case "needs_reconciliation": return "Sprawdź wynik"
        case "signing": return "Podpisywanie na PC"
        case "provisioning": return "Przygotowywanie profilu"
        case "installing": return "Instalowanie"
        case "verifying": return "Potwierdzanie na urządzeniu"
        default: return "Przygotowywanie"
        }
    }
}
enum StoreError: LocalizedError {
    case message(String)
    var errorDescription: String? { switch self { case .message(let text): return text } }
}

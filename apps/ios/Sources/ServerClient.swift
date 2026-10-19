import Foundation
import Security
import CryptoKit

final class PinnedConnection: NSObject, URLSessionDelegate, URLSessionTaskDelegate {
    let pairing: Pairing
    init(_ pairing: Pairing) { self.pairing = pairing }
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              challenge.protectionSpace.host == URL(string: pairing.url)?.host,
              challenge.protectionSpace.port == (URL(string: pairing.url)?.port ?? 443),
              let trust = challenge.protectionSpace.serverTrust,
              let certificate = SecTrustGetCertificateAtIndex(trust, 0) else {
            completionHandler(.cancelAuthenticationChallenge, nil); return
        }
        let data = SecCertificateCopyData(certificate) as Data
        let fingerprint = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard fingerprint == pairing.certificateSHA256.lowercased() else {
            completionHandler(.cancelAuthenticationChallenge, nil); return
        }
        // Trust only the exact certificate approved in the PC's QR, with the SSL host policy.
        guard SecTrustSetAnchorCertificates(trust, [certificate] as CFArray) == errSecSuccess,
              SecTrustSetAnchorCertificatesOnly(trust, true) == errSecSuccess,
              SecTrustEvaluateWithError(trust, nil) else {
            completionHandler(.cancelAuthenticationChallenge, nil); return
        }
        completionHandler(.useCredential, URLCredential(trust: trust))
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}

final class ServerClient {
    let pairing: Pairing
    private let session: URLSession
    private let delegate: PinnedConnection
    private let decoder: JSONDecoder = { let decoder = JSONDecoder(); decoder.keyDecodingStrategy = .convertFromSnakeCase; return decoder }()
    init(pairing: Pairing) throws {
        try pairing.validate()
        self.pairing = pairing
        delegate = PinnedConnection(pairing)
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 45
        config.timeoutIntervalForResource = 900
        config.httpShouldSetCookies = false
        config.urlCache = nil
        session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
    }
    deinit { session.invalidateAndCancel() }
    private func request(_ path: String, method: String = "GET") throws -> URLRequest {
        guard let base = URL(string: pairing.url) else { throw StoreError.message("Nieprawidłowy adres PC.") }
        var request = URLRequest(url: base.appendingPathComponent(path))
        request.httpMethod = method
        request.setValue("Bearer " + pairing.token, forHTTPHeaderField: "Authorization")
        return request
    }
    private func checked(_ data: Data, _ response: URLResponse) throws -> Data {
        guard let http = response as? HTTPURLResponse else { throw StoreError.message("Nieprawidłowa odpowiedź serwera.") }
        guard (200..<300).contains(http.statusCode) else {
            if http.statusCode == 401 { throw StoreError.message("PC odwołał zaufanie. Połącz MostStore ponownie kodem QR.") }
            let message = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["message"] as? String
            throw StoreError.message(message ?? "Operacja nie powiodła się (HTTP \(http.statusCode)).")
        }
        return data
    }
    func state() async throws -> StoreSnapshot {
        let (data, response) = try await session.data(for: request("v1/state"))
        let state = try decoder.decode(StoreSnapshot.self, from: checked(data, response))
        guard state.deviceId == pairing.deviceID else { throw StoreError.message("Odpowiedź PC dotyczy innego urządzenia.") }
        return state
    }
    func install(_ artifact: Artifact, mode: String, key: String) async throws {
        var req = try request("v1/jobs", method: "POST")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue(key, forHTTPHeaderField: "Idempotency-Key")
        req.httpBody = try JSONSerialization.data(withJSONObject: ["artifact_id": artifact.id, "device_id": pairing.deviceID, "mode": mode])
        let (data, response) = try await session.data(for: req)
        _ = try checked(data, response)
    }
    func reconcile(_ job: InstallJob) async throws {
        let (data, response) = try await session.data(for: request("v1/jobs/\(job.id)/reconcile", method: "POST"))
        _ = try checked(data, response)
    }
    func upload(_ file: URL) async throws {
        let boundary = "MostStore-" + UUID().uuidString
        let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".multipart")
        guard FileManager.default.createFile(atPath: temporary.path, contents: nil) else { throw StoreError.message("Nie można utworzyć pliku transferu.") }
        defer { try? FileManager.default.removeItem(at: temporary) }
        let output = try FileHandle(forWritingTo: temporary)
        defer { try? output.close() }
        try output.write(contentsOf: Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"ipa\"; filename=\"application.ipa\"\r\nContent-Type: application/octet-stream\r\n\r\n".utf8))
        let input = try FileHandle(forReadingFrom: file)
        defer { try? input.close() }
        var total: UInt64 = 0
        while let chunk = try input.read(upToCount: 1024 * 1024), !chunk.isEmpty {
            total += UInt64(chunk.count)
            guard total <= 2 * 1024 * 1024 * 1024 else { throw StoreError.message("IPA przekracza limit 2 GiB.") }
            try output.write(contentsOf: chunk)
        }
        try output.write(contentsOf: Data("\r\n--\(boundary)--\r\n".utf8))
        try output.synchronize()
        var req = try request("v1/artifacts", method: "POST")
        req.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        let (data, response) = try await session.upload(for: req, fromFile: temporary)
        _ = try checked(data, response)
    }
}

enum PairingKeychain {
    static let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "org.moststore.connection", kSecAttrAccount as String: "active"]
    static func load() -> Pairing? {
        var request = query; request[kSecReturnData as String] = true; request[kSecMatchLimit as String] = kSecMatchLimitOne
        var output: CFTypeRef?
        guard SecItemCopyMatching(request as CFDictionary, &output) == errSecSuccess, let data = output as? Data else { return nil }
        return try? JSONDecoder().decode(Pairing.self, from: data)
    }
    static func save(_ pairing: Pairing) throws {
        let data = try JSONEncoder().encode(pairing)
        let update = [kSecValueData as String: data]
        let result = SecItemUpdate(query as CFDictionary, update as CFDictionary)
        if result == errSecItemNotFound {
            var entry = query; entry[kSecValueData as String] = data
            entry[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            guard SecItemAdd(entry as CFDictionary, nil) == errSecSuccess else { throw StoreError.message("Nie można zapisać połączenia w Keychain.") }
        } else if result != errSecSuccess { throw StoreError.message("Nie można zaktualizować połączenia w Keychain.") }
    }
    static func remove() { SecItemDelete(query as CFDictionary) }
}

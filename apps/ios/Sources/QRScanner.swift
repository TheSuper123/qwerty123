import SwiftUI
import UIKit
import AVFoundation

struct QRScanner: UIViewControllerRepresentable {
    let completion: (Result<String, Error>) -> Void
    func makeUIViewController(context: Context) -> ScannerController { ScannerController(completion: completion) }
    func updateUIViewController(_ uiViewController: ScannerController, context: Context) {}
    static func dismantleUIViewController(_ uiViewController: ScannerController, coordinator: ()) { uiViewController.stop() }
}

final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?
    private let completion: (Result<String, Error>) -> Void
    private var finished = false
    private let cameraQueue = DispatchQueue(label: "org.moststore.camera")
    private var stopped = false
    init(completion: @escaping (Result<String, Error>) -> Void) { self.completion = completion; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError("ScannerController is created programmatically") }
    override func viewDidLoad() {
        super.viewDidLoad(); view.backgroundColor = .black
        AVCaptureDevice.requestAccess(for: .video) { [weak self] allowed in
            guard let self else { return }
            if allowed { self.cameraQueue.async { self.configure() } }
            else { DispatchQueue.main.async { self.finish(.failure(StoreError.message("Włącz dostęp do kamery albo wklej konfigurację QR ręcznie."))) } }
        }
    }
    private func configure() {
        guard !stopped else { return }
        do {
            guard let camera = AVCaptureDevice.default(for: .video) else { throw StoreError.message("Brak dostępnej kamery. Wklej konfigurację ręcznie.") }
            let input = try AVCaptureDeviceInput(device: camera)
            guard session.canAddInput(input) else { throw StoreError.message("Nie można uruchomić kamery.") }
            session.addInput(input)
            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else { throw StoreError.message("Nie można uruchomić skanowania QR.") }
            session.addOutput(output); output.setMetadataObjectsDelegate(self, queue: .main); output.metadataObjectTypes = [.qr]
            DispatchQueue.main.async {
                let layer = AVCaptureVideoPreviewLayer(session: self.session); layer.videoGravity = .resizeAspectFill; layer.frame = self.view.bounds
                self.preview = layer; self.view.layer.addSublayer(layer)
            }
            session.startRunning()
        } catch { DispatchQueue.main.async { self.finish(.failure(error)) } }
    }
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); preview?.frame = view.bounds }
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        if let value = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue { finish(.success(value)) }
    }
    private func finish(_ result: Result<String, Error>) { guard !finished else { return }; finished = true; stop(); completion(result) }
    func stop() { cameraQueue.async { self.stopped = true; if self.session.isRunning { self.session.stopRunning() } } }
}

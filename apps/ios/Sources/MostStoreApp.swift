import SwiftUI

@main
@MainActor
struct MostStoreApp: App {
    @StateObject private var model = StoreModel()
    var body: some Scene {
        WindowGroup { RootView().environmentObject(model).tint(Color.mostGreen).preferredColorScheme(.dark) }
    }
}

extension Color {
    static let mostGreen = Color(red: 0.82, green: 0.96, blue: 0.55)
    static let mostBackground = Color(red: 0.06, green: 0.09, blue: 0.07)
}

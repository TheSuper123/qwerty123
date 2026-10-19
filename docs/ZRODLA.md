# Źródła i granice potwierdzenia

Przegląd dokumentacji: 19 października 2026. Sprawdzono teksty w publicznych repozytoriach poniżej. Nie uruchamiano AltServer, SideStore, LocalDevVPN ani instalacji iOS w tym środowisku Linux. Ten przegląd uzasadnia plan, nie potwierdza kompatybilności Most.

## Dokumentacja odczytana

| Źródło | Co potwierdza na potrzeby planu |
| --- | --- |
| [AltStore — README](https://github.com/altstoreio/AltStore/blob/main/README.md) | Model klient + serwer, podpisywanie, Wi-Fi, odnowienia i zależność od AltSign; AGPLv3 z zastrzeżeniami opisanymi przez autorów |
| [AltServer Windows — README](https://github.com/rileytestut/AltServer-Windows/blob/master/README.md) | Istnienie natywnej ścieżki Windows i zależności podpisywania/urządzeń; historyczna instrukcja, nie gotowa recepta dla Most |
| [SideStore — README](https://github.com/SideStore/SideStore/blob/develop/README.md) | Rola lokalnego VPN, LocalDevVPN, em_proxy i minimuxer; opis AGPLv3 |
| [LocalDevVPN — README](https://github.com/jkcoxson/LocalDevVPN/blob/master/README.md) | Lokalny wirtualny interfejs bez zewnętrznego serwera; dystrybucja App Store wskazana przez autora |
| [em_proxy — README](https://github.com/jkcoxson/em_proxy/blob/master/README.md) | Proxy pakietów do obsługi ograniczeń loopback; samodzielnie nie zapewnia podpisywania |
| [minimuxer — README](https://github.com/jkcoxson/minimuxer/blob/master/README.md) | Lokalny muxer, komunikacja z kodem Swift i bibliotekami urządzenia |
| [pymobiledevice3 — README](https://github.com/doronz88/pymobiledevice3/blob/master/README.md) | Biblioteka i CLI dla Windows/macOS/Linux, profile i aplikacje, opis GPL-3.0 |
| [pymobiledevice3 — instalacja](https://github.com/doronz88/pymobiledevice3/blob/master/docs/installation.md) | Znaczenie Apple Mobile Device Service i różnic wykrywania Wi-Fi na Windows |
| [pymobiledevice3 — tunele iOS 17+](https://github.com/doronz88/pymobiledevice3/blob/master/docs/guides/ios17-tunnels.md) | CoreDevice/RSD, różnice wersji iOS, tunel userspace a tunneld; ostrzeżenie o nieuwierzytelnionym API tunneld |

Odnośniki wskazują zmienne gałęzie, nie przypięte rewizje. Przy wyborze zależności w etapie 0 zapisujemy commit/release, pełną licencję, sumy artefaktów i zgodną instrukcję budowania. Sam README nie ustala wszystkich warunków licencyjnych.

## Dokumenty Apple do ponownej weryfikacji w etapie 0

- [Porównanie rodzajów członkostwa](https://developer.apple.com/support/compare-memberships/).
- [Packet Tunnel Provider](https://developer.apple.com/documentation/networkextension/packet-tunnel-provider).
- [Developer Mode](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device).
- [Network Extensions entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.networking.networkextension).

Bezpośredni dostęp do dwóch pierwszych dokumentów zwrócił w środowisku proxy `403`; pozostałe podano jako materiały do weryfikacji, bez twierdzenia, że zostały tu odczytane. Z tego powodu aktualne warunki kont i capabilities muszą być sprawdzone podczas prototypu na rzeczywistym koncie.

## Ograniczenia przyjęte do projektu

1. **Personal Team:** typowy okres ważności profilu wynosi 7 dni, a często spotykane ograniczenia to 3 aktywne aplikacje deweloperskie na urządzeniu i 10 App ID w 7 dni. MostStore również zużywa miejsce w limicie; rozszerzenia mogą wymagać dodatkowych App ID. Nie traktujemy tych liczb jako wiecznego kontraktu Apple. Pokazujemy rzeczywisty profil, odpowiedzi usług i wyniki testów konta.
2. **Płatne konto:** zwykle daje dłuższe profile, lecz nie odblokowuje automatycznie dowolnego entitlementu, dowolnej IPA ani każdej metody dystrybucji. Termin czytamy z konkretnego profilu.
3. **LocalDevVPN:** lokalny tunel nie łączy automatycznie z PC. Dodatkowy kanał Remote i dostęp do usług własnego urządzenia muszą zadziałać razem.
4. **Własny VPN:** potrzebne są właściwe capabilities, profile i dozwolona dystrybucja. Nie obiecujemy go w darmowo podpisanej aplikacji Personal Team.
5. **Praca w tle:** iOS nie daje zwykłej aplikacji gwarancji stałego działania, regularnego odnowienia lub działania po wygaśnięciu podpisu.
6. **Samoodnowienie klienta:** relaya może zabić aktualizacja jego własnej aplikacji. To osobna bramka eksperymentalna, z PC USB/LAN jako ratunkiem.
7. **JIT i developer tunnel:** to dodatkowe przepływy z własnymi wymaganiami. Nie są reklamowane jako automatyczny skutek instalowania IPA.
8. **Dystrybucja UE:** oficjalny marketplace i uprawnienia związane z alternatywną dystrybucją to inny model; nie są założeniem Most.

## Co potwierdza obecne repozytorium

- Istnieje spójny plan produktu, architektury, API i etapów implementacji.
- Istnieje statyczna strona oraz konfiguracja jej wdrożenia na GitHub Pages.
- Implementacja preview ma serwer Rust z integracją isideload 0.5.0 / idevice 0.1.68, panel i klienta MostStore LAN w SwiftUI. Wersje i źródła są przypięte w Cargo.lock, a licencje zebrane w THIRD_PARTY_NOTICES.md i paczkach.
- [isideload](https://github.com/nab138/isideload) oraz [idevice](https://github.com/jkcoxson/idevice) udostępniają obsługę konta, podpisywania i urządzeń. Odczytano ich dokumentację i API; ograniczona obsługa entitlementów w isideload jest uwzględniona w ograniczeniach preview.
- Nie ma jeszcze certyfikacji kompatybilności, wyniku testów na iPhonie ani implementacji relaya Remote/LocalDevVPN.

Test strony i dokumentacji nie jest testem sideloadingu. Wyniki przyszłych prób urządzeń powinny być zapisywane osobno z dokładnymi wersjami i statusem każdego scenariusza.

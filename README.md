# Most

**Twój PC. Twoje aplikacje. Twój most do iPhone’a.**

Most to otwarte narzędzie do instalowania i odnawiania własnych aplikacji iOS: **MostServer** działa na komputerze, a **MostStore** na iPhonie. Podpisywanie i prowadzenie instalacji pozostają po stronie PC. Pierwsza implementacja obsługuje USB/Wi-Fi do urządzenia i klienta LAN; relay zdalny jest w planie.

> **Stan: preview 0.1.x.** Zaimplementowano MostServer w Rust, lokalny panel, integrację podpisywania/instalacji i klienta MostStore LAN w SwiftUI. Test na fizycznym iPhonie, Remote i LocalDevVPN pozostają do wykonania. Workflow natywnie buduje EXE/ZIP Windows i niepodpisaną IPA na macOS; dopiero udany build aktywuje pobieranie na stronie.

## Pobierz i uruchom

Otwórz [stronę pobierania](https://thesuper123.github.io/qwerty123/#pobierz) lub [wydania GitHub](https://github.com/TheSuper123/qwerty123/releases). Pobierz ZIP, rozpakuj i uruchom `MostServer.exe`. Program otworzy lokalny panel w przeglądarce i nie wymaga Pythona.

Wymagane są sterowniki Apple. Podpisywanie kontem Apple wymaga dodatkowo przygotowania usługi anisette v3; domyślnie własnej na PC. `MostStore-unsigned.ipa` trzeba podpisać własnym kontem przed instalacją. **[Pełna instrukcja Windows i iPhone](docs/PROGRAM_WINDOWS.md)** opisuje te kroki oraz aktualne ograniczenia. Brak podpisu wydawcy Windows; to pierwsza wersja preview, a nie komplet funkcji z docelowego planu.

Kod serwera i testy:

```sh
cargo test --locked --workspace
cargo run --locked --bin MostServer
```

Wersję Rust przypina `rust-toolchain.toml`, a zależności `Cargo.lock`. Budowanie iOS: `apps/ios/project.yml` i XcodeGen na Macu.

## Założenia

- **Windows 11 x64 jako pierwszy serwer**, później macOS; bez obowiązkowej chmury.
- Instalacja IPA przez USB, następnie przez domowe Wi-Fi.
- **MostLink**: planowane połączenie zdalne z własnym PC, z uwierzytelnianiem obu urządzeń.
- **MostTunnel**: adapter lokalnego tunelu iOS, początkowo z zewnętrznym LocalDevVPN. Własne rozszerzenie VPN to osobny etap wymagający odpowiednich uprawnień Apple.
- Profile, klucze podpisujące i obsługa konta Apple na komputerze; bez publicznego serwera anisette w konfiguracji domyślnej.
- Zwykłe ograniczenia podpisywania Apple nadal obowiązują. VPN nie przedłuża ważności aplikacji i nie zwiększa limitów konta.

**Remote API, dostęp do usług iPhone’a oraz lokalny VPN to trzy osobne elementy.** Sama dostępność serwera przez internet nie umożliwia instalacji na telefonie. Zdalny most oraz odnawianie samego MostStore wymagają prototypu opisanego w planie.

## Pełny plan

| Dokument | Zawartość |
| --- | --- |
| [Program Windows i MostStore LAN](docs/PROGRAM_WINDOWS.md) | Uruchomienie, sterowniki, anisette, pierwsza instalacja i aktualne API |
| [Plan produktu](docs/PLAN_PRODUKTU.md) | Nazwa, zakres, ekrany, scenariusze, wymagania i granice projektu |
| [Architektura](docs/ARCHITEKTURA.md) | Komponenty, podpisywanie, USB/LAN, MostLink, LocalDevVPN i bezpieczeństwo |
| [Kontrakt API](docs/API.md) | Parowanie, operacje, zdarzenia, błędy i kanał zdalny |
| [Roadmapa](docs/ROADMAPA.md) | Kolejność prac, bramki wykonalności, testy, zasoby i kryteria wydania |
| [Źródła i ograniczenia](docs/ZRODLA.md) | Dokumentacja źródłowa, zależności, licencje i granice potwierdzenia |

## Strona projektu

Źródła strony znajdują się w [`site/`](site/). Strona nie wymaga Node.js ani instalowania pakietów:

```sh
cd /workspace/qwerty123
python3 -m http.server 8080 --directory site --bind 127.0.0.1
```

Otwórz serwowany adres w lokalnej przeglądarce. Zatrzymaj serwer przez `Ctrl+C`.

Workflow [GitHub Pages](.github/workflows/pages.yml) publikuje wyłącznie katalog `site/` po wysłaniu zmian na `main` lub ręcznym uruchomieniu. Docelowy adres dla obecnej nazwy repozytorium: `https://thesuper123.github.io/qwerty123/`. Samo dodanie workflow nie włącza witryny Pages.

### Pierwsza publikacja

1. Właściciel repozytorium otwiera [Settings → Pages](https://github.com/TheSuper123/qwerty123/settings/pages).
2. W sekcji **Build and deployment** ustawia **Source → GitHub Actions**. Nie wybiera `Deploy from a branch`.
3. Po zapisaniu wraca do [Actions → Publish Most to GitHub Pages](https://github.com/TheSuper123/qwerty123/actions/workflows/pages.yml).
4. Wybiera **Run workflow → Branch: main → Run workflow**. Trzeba uruchomić workflow ponownie po zmianie ustawienia; wcześniejszy czerwony wynik nie zmieni się sam.
5. Czeka na zielone zadania `build` i `deploy`. Dopiero zakończone wdrożenie potwierdza publikację strony.

### Błąd „Get Pages site failed / Not Found”

Ten błąd w `actions/configure-pages` oznacza, że token workflow nie może odczytać witryny Pages. Przy pierwszej publikacji należy najpierw aktywować Pages według instrukcji powyżej. Jeśli witryna jest już aktywna, trzeba sprawdzić dostęp workflow do Pages oraz dostępność tej funkcji dla repozytorium i planu GitHub.

Krok `Verify Pages activation` sprawdza dostępność konfiguracji i źródło `workflow` przed publikacją. Przy problemie kończy zadanie błędem i zapisuje konkretną instrukcję w podsumowaniu uruchomienia. Nie zmienia ustawień repozytorium.

Samo ustawienie `enablement: true` nie rozwiązuje aktywacji ze standardowym `GITHUB_TOKEN`: [kontrakt akcji configure-pages](https://github.com/actions/configure-pages/blob/v5/action.yml) wymaga w takim trybie dodatkowego tokena z uprawnieniami administracyjnymi. Ten projekt korzysta ze standardowego tokena workflow; pierwszą aktywację wykonuje właściciel w ustawieniach, bez dodawania sekretów.

## Najbliższy cel

Etap 0: na fizycznym iPhonie udowodnić podpisanie i instalację niewielkiej własnej IPA z Windows, a następnie sprawdzić lokalny most instalacyjny i zachowanie przy aktualizacji samego klienta. Dopiero pozytywny wynik otwiera implementację pełnego trybu zdalnego.

Nazwa **Most** jest robocza; przed publicznym wydaniem trzeba sprawdzić dostępność nazwy i znaków. To niezależny projekt, bez powiązania z Apple, AltStore, SideStore ani twórcami LocalDevVPN.

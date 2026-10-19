# Most

**Twój PC. Twoje aplikacje. Twój most do iPhone’a.**

Most to plan otwartego narzędzia do instalowania i odnawiania własnych aplikacji iOS: **MostServer** działa na komputerze, a **MostStore** na iPhonie. Podpisywanie i prowadzenie instalacji pozostają po stronie PC. Telefon wybiera aplikację, pokazuje postęp i, w trybie zdalnym, udostępnia kontrolowany most transportowy.

> **Stan: projekt i plan techniczny.** Repozytorium zawiera dokumentację oraz działającą statyczną stronę projektu. MostServer, MostStore i protokół instalacji nie są jeszcze zaimplementowane ani przetestowane na urządzeniach.

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

Workflow [GitHub Pages](.github/workflows/pages.yml) publikuje wyłącznie katalog `site/` po wysłaniu zmian na `main` lub ręcznym uruchomieniu. W ustawieniach repozytorium wybierz **Pages → Source → GitHub Actions**. Docelowy adres dla obecnej nazwy repozytorium: `https://thesuper123.github.io/qwerty123/`. Samo zapisanie plików nie oznacza, że strona jest już opublikowana.

## Najbliższy cel

Etap 0: na fizycznym iPhonie udowodnić podpisanie i instalację niewielkiej własnej IPA z Windows, a następnie sprawdzić lokalny most instalacyjny i zachowanie przy aktualizacji samego klienta. Dopiero pozytywny wynik otwiera implementację pełnego trybu zdalnego.

Nazwa **Most** jest robocza; przed publicznym wydaniem trzeba sprawdzić dostępność nazwy i znaków. To niezależny projekt, bez powiązania z Apple, AltStore, SideStore ani twórcami LocalDevVPN.

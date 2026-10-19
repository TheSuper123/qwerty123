# Most — plan produktu

Status: specyfikacja przed implementacją. Data opracowania: 19 października 2026.

## 1. Nazwa i obietnica

**Most** łączy własny komputer z iPhonem. Hasło: **„Twój PC. Twoje aplikacje.”**

| Nazwa | Rola |
| --- | --- |
| MostServer | Program i ikona w zasobniku Windows; konto Apple, podpisywanie, instalacja, kolejka |
| MostStore | Aplikacja SwiftUI; biblioteka IPA, źródła aplikacji, terminy ważności, zlecenia |
| MostLink | Uwierzytelniony kanał sterowania i transport zdalny pomiędzy telefonem a PC |
| MostTunnel | Adapter lokalnego mostu do usług urządzenia, współpracujący z LocalDevVPN |

MostStore nie będzie forkiem interfejsu AltStore. Pierwsza wersja ma własny wygląd i mały zakres. Wykorzystanie istniejących bibliotek wymaga zgodności z ich licencjami.

## 2. Dla kogo i jaki problem rozwiązuje

Użytkownik ma Windows i iPhone’a, chce instalować własne lub legalnie udostępnione IPA i widzieć, kiedy trzeba je odnowić. Nie chce przekazywać konta Apple operatorowi obcej usługi podpisującej ani utrzymywać serwera VPS.

Komputer pozostaje potrzebny do każdej operacji podpisywania i instalowania. Telefon poza domem może zlecić operację tylko wtedy, gdy komputer działa i istnieje sprawny kanał do usług iPhone’a. Bez tego operacja czeka; aplikacja nie udaje sukcesu.

## 3. Zakres kolejnych wydań

| Wydanie | Wchodzi w zakres | Warunek |
| --- | --- | --- |
| Prototyp | Podpis jednej testowej IPA i instalacja USB z Windows | Fizyczne urządzenie i rzeczywiste konto Apple |
| MVP | Bootstrap MostStore, import IPA, lista urządzeń, instalacja/odnowienie USB oraz LAN, historia i terminy | Działający signer i zaufanie urządzenia |
| Beta Remote | Połączenie przez internet, lokalny most iOS, odtwarzanie sesji, diagnostyka | Zaliczona bramka Remote z roadmapy |
| Wersja 1.0 | Sprawdzone odzyskiwanie, dystrybucja Windows, dokumentacja, audyt zależności | Brak krytycznych usterek w macierzy testów |
| Później | Własny provider VPN, serwer macOS, wiele urządzeń, wybrane funkcje deweloperskie | Osobna ocena uprawnień i wykonalności |

Tryb zdalny i integracja LocalDevVPN są częścią docelowego produktu. Oddzielenie ich od MVP pozwala uzyskać działający instalator wcześniej; nie oznacza usunięcia ich z planu.

Poza pierwszą wersją: jailbreak, omijanie DRM, pobieranie płatnych aplikacji, sklep z pirackimi IPA, wymuszanie prywatnych entitlementów, gwarantowany JIT, własny marketplace UE, usługa podpisująca dla wielu obcych użytkowników i instalacja przy wyłączonym PC.

## 4. Decyzje platformowe

- Serwer: Windows 11 x64 uruchomiony natywnie. Docker i WSL nie są domyślną ścieżką obsługi USB.
- Klient: kandydat na minimalny system to iOS 17.4; rzeczywista lista wspieranych wydań powstanie po testach. Nie deklarujemy wszystkich nowszych iOS jako zgodnych z góry.
- Budowanie klienta: macOS i zgodny Xcode są potrzebne programiście lub runnerowi CI. Użytkownik gotowej IPA potrzebuje tylko PC i iPhone’a.
- Konto bezpłatne i płatne mają osobne scenariusze testów. Konto płatne nie gwarantuje uprawnień do każdej funkcji IPA.
- Pierwsze zaufanie komputerowi i bootstrap przez kabel. Nie obiecujemy pierwszej instalacji na niesparowanym telefonie wyłącznie przez internet.
- Jeden użytkownik i jedno aktywnie obsługiwane urządzenie w MVP; model danych przygotowany na więcej.

## 5. Pierwsze uruchomienie

1. Instalator PC sprawdza system, miejsce na dysku i obsługę Apple Mobile Device. Podaje źródło sterowników; nie pobiera przypadkowych paczek DLL.
2. Użytkownik podłącza odblokowany iPhone i potwierdza „Zaufaj”. Kreator sprawdza rzeczywistą sesję lockdown.
3. Użytkownik loguje się do Apple na PC, przechodzi 2FA i wybiera zespół. Dane nie trafiają do MostStore ani do logów.
4. MostServer podpisuje MostStore właściwym profilem i instaluje go przez USB.
5. Kreator prowadzi przez wymagane przez dany iOS zaufanie profilowi i Developer Mode, wraz z restartem, jeżeli jest konieczny.
6. MostStore skanuje kod QR z PC. Powstaje osobne zaufanie aplikacja–serwer; nie zastępuje ono parowania Apple.
7. Test końcowy: lista aplikacji i udane zlecenie instalacji własnej IPA. Ekran pokazuje rzeczywisty termin ważności profilu.

W wersji Remote dochodzi kreator zewnętrznego LocalDevVPN oraz test z telefonu poza domowym Wi-Fi. Przycisk „Remote gotowe” pojawia się po teście transportu i instalacji, a nie po samym połączeniu HTTPS.

## 6. Zwykła instalacja i odnowienie

1. IPA pochodzi z pliku wybranego przez użytkownika lub jawnie dodanego źródła. Klient pokazuje autora, wersję, rozmiar, uprawnienia i skrót pliku.
2. PC sprawdza archiwum, platformę, zaszyfrowanie, identyfikatory oraz zgodność uprawnień z kontem.
3. Użytkownik akceptuje operację. MostServer przygotowuje profil, podpisuje binaria i zapisuje wynik zadania.
4. Instalacja korzysta z USB, dostępnego LAN albo zatwierdzonego mostu Remote. Podpis nie przemieszcza się do telefonu jako prywatny klucz.
5. Sukces wymaga odpowiedzi usługi instalacyjnej i potwierdzenia obecności właściwej wersji/bundle ID. Na urządzeniu testowym dodatkowo sprawdzamy ręcznie uruchomienie aplikacji.

Odnowienie używa zapisanego oryginału IPA i stabilnego mapowania bundle ID, aby zachować tożsamość aplikacji. Nie usuwamy aplikacji przed odnowieniem. Nie obiecujemy zachowania danych przy zmianie zespołu, identyfikatora lub uprawnień.

## 7. Ekrany MostStore

| Ekran | Zawartość i działanie |
| --- | --- |
| Start | Stan PC, wybrany transport, liczba aplikacji do odnowienia, ostatni wynik |
| Moje aplikacje | Ikona, wersja, termin profilu, „Odnowić”, „Szczegóły” |
| Biblioteka | Własne IPA i ręcznie dodane źródła; bez domyślnego katalogu nieznanych aplikacji |
| Instalacja | Etap zadania, rzeczywisty postęp transferu, możliwość bezpiecznego anulowania |
| Połączenie | Zaufany PC, LAN/Remote, test kanału urządzenia, stan lokalnego tunelu |
| Diagnostyka | Kod błędu, konkretna instrukcja naprawy, eksport raportu bez sekretów |
| Ustawienia | Powiadomienia, limity danych komórkowych, źródła, odwołanie zaufania |

Ekrany PC: kreator konfiguracji, urządzenia, konto i zespół, biblioteka IPA, kolejka, certyfikaty/profil, konfiguracja Remote i diagnostyka. Operacje konta wymagające obecności użytkownika pojawiają się na PC.

## 8. Automatyzacja i ograniczenia iOS

Domyślnie przypominamy 48 i 24 godziny przed odczytanym terminem ważności; terminy są ustawieniem produktu, nie gwarancją systemu. Background Tasks wykonują tylko krótkie próby, gdy iOS na to pozwoli. Nie utrzymujemy sztucznie aplikacji w tle przez audio ani nie zakładamy stałego WebSocketu.

Instalacja Remote wymaga otwartego MostStore, aktywnego tunelu i działającego PC. Po zawieszeniu klienta zadanie jest wstrzymywane albo uzgadniane po ponownym połączeniu. Instalacja już zatwierdzona przez iOS może wymagać osobnej weryfikacji wyniku.

Szczególny przypadek: aktualizacja **samego MostStore** może zakończyć proces będący mostem. Dopóki prototyp nie udowodni niezależnego dokończenia operacji przez system, odnawianie klienta odbywa się przez USB/LAN z PC. Po wygaśnięciu MostStore ścieżką ratunkową pozostaje PC i kabel.

## 9. Wymagania mierzalne

- Stan „zainstalowano” nigdy nie wynika tylko z udanego podpisania lub wysłania pliku.
- Nie pokazujemy wymyślonego procentu podczas podpisywania; postęp procentowy tylko tam, gdzie znamy liczbę bajtów lub elementów.
- Jedno aktywne zadanie modyfikujące dane urządzenia. Ponowienie tego samego żądania nie tworzy dwóch instalacji.
- Po rozłączeniu zachowujemy etap i skróty; nie odtwarzamy automatycznie nieznanego wyniku instalacji.
- Skanowanie QR, potwierdzenie zaufania i pierwsza diagnoza błędu mają działać bez terminala.
- Interfejs polski i angielski, VoiceOver, Dynamic Type, obsługa klawiatury w PC.
- Startowy limit IPA: 2 GiB, regulowany przez użytkownika. Kontrola wolnego miejsca i bezpieczny rozmiar po rozpakowaniu są obowiązkowe.
- Brak telemetrii włączonej domyślnie. Eksport diagnostyki wymaga świadomego wyboru.

## 10. Kryterium realizacji pomysłu

Projekt spełnia całą wizję, gdy użytkownik z własnym PC potrafi zainstalować i odnowić własną IPA przez USB, Wi-Fi oraz z zewnętrznej sieci z działającym lokalnym mostem iOS, a błędy i ograniczenia są jasno pokazane. Udane USB/LAN oznacza gotowe MVP, ale nie potwierdza gotowości Remote ani samoodnawiania klienta.

Dalsze szczegóły: [architektura](ARCHITEKTURA.md), [API](API.md), [roadmapa i testy](ROADMAPA.md).

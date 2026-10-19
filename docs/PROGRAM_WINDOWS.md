# MostServer 0.1.0 i MostStore LAN — instrukcja preview

Repozytorium zawiera implementację, testy i workflow tworzący przenośne EXE/ZIP Windows oraz niepodpisaną IPA klienta iOS. Publikacja paczek jest warunkowana udanym CI. Weryfikacja instalacji na fizycznym iPhonie i podpis wydawcy Windows nie zostały wykonane w środowisku deweloperskim Linux.

## Co jest zaimplementowane

- Jeden proces MostServer z lokalnym panelem w przeglądarce, bez Pythona na komputerze użytkownika.
- Import IPA, SHA-256, deduplikacja, odczyt `Info.plist`, wykrywanie DRM, limity rozmiaru i zabezpieczone rozpakowanie.
- Wykrywanie przez usługę Apple, parowanie USB, lista aplikacji i potwierdzanie numeru buildu.
- Instalacja już podpisanej IPA przez usługi urządzenia.
- Integracja `isideload` 0.5.0: logowanie Apple, 2FA urządzenie/SMS, zespoły, provisioning i podpisywanie podstawowych aplikacji bez rozszerzeń.
- Trwała historia SQLite, idempotentne przyjmowanie zleceń, stan niepewny po przerwaniu instalacji i uzgadnianie wyniku.
- Zaszyfrowany magazyn certyfikatów/anisette/TLS; na Windows klucz główny znajduje się w Credential Manager. Hasło Apple nie jest zapisywane, sesja jest w pamięci.
- Opcjonalny interfejs MostStore LAN na prywatnym IPv4: HTTPS, przypięcie certyfikatu po stronie iOS, odwoływalny klucz przypisany do urządzenia.
- MostStore w SwiftUI: skan QR lub wklejenie konfiguracji, biblioteka, import IPA z Plików, zlecanie instalacji, historia, uzgadnianie wyników i Keychain.

Niezaimplementowane: logowanie bez przygotowanej usługi anisette, automatyczne odnowienia w tle, relay poza LAN, integracja LocalDevVPN, własny provider VPN i JIT. Wydanie nie jest pełnym zamiennikiem AltServer/AltStore. Kod obsługi urządzenia nie oznacza zaliczenia testów sprzętowych.

## 1. Pobierz i uruchom program

1. Na stronie projektu otwórz **Pobierz**. Aktywne odnośniki pojawiają się po udanych buildach i opublikowaniu GitHub Release. Przy stanie oczekiwania sprawdź workflow **Build and release Most**.
2. Pobierz `MostServer-windows-x64.zip`, rozpakuj i uruchom `MostServer.exe`. ZIP zawiera również instrukcję oraz licencje zależności. Alternatywnie pobierz sam EXE; pełne informacje licencyjne znajdziesz w ZIP i źródłach.
3. Program otwiera panel w domyślnej przeglądarce. Domyślny port panelu to `49240`, wyłącznie na `127.0.0.1`.
4. Zostaw okno procesu otwarte. Zamknięcie karty nie zamyka serwera. Zakończenie: przycisk w Diagnostyce lub `Ctrl+C`.

Okno procesu zawiera odnośnik z losowym lokalnym kluczem dostępu. Nie wysyłaj go w logach ani na zrzutach ekranu. Panel usuwa klucz z paska adresu i przechowuje go tylko w `sessionStorage` bieżącej sesji przeglądarki.

EXE nie ma certyfikatu wydawcy Windows. Suma SHA-256 w `SHA256SUMS.txt` pozwala sprawdzić integralność pobranego pliku, nie zastępuje podpisu wydawcy. Paczki pobieraj z repozytorium projektu przez HTTPS.

## 2. Sterowniki i urządzenie

Zainstaluj oficjalne [iTunes / obsługę urządzeń Apple](https://www.apple.com/itunes/download/win64). Most nie dołącza nieoficjalnych sterowników DLL. Usługa **Apple Mobile Device Service** musi działać.

Podłącz odblokowany iPhone kablem z transmisją danych, otwórz **Urządzenia → Sprawdź urządzenia** i potwierdź **Zaufaj**. Przycisk **Sparuj / Zaufaj** czeka do 90 sekund na zgodę w telefonie. Developer Mode i zaufanie profilowi trzeba włączyć, jeśli wymaga tego iOS; program ich nie omija.

Wi-Fi działa, jeśli usługom Apple udaje się wykryć wcześniej sparowane urządzenie. Włącz synchronizację Wi-Fi w iTunes. Samo połączenie obu urządzeń z routerem nie jest gwarancją dostępności usług instalacyjnych. Brak sterowników i brak telefonu są pokazywane osobno.

## 3. Dwa sposoby instalowania IPA

**Już podpisana IPA:** zaimportuj plik do Biblioteki, wybierz urządzenie i tryb „IPA jest już poprawnie podpisana”. Potrzebny jest ważny podpis i profil dla tego urządzenia. Odczytane daty profilu są metadanymi; ostateczną zgodność sprawdza iOS.

**Podpisz moim kontem Apple:** przygotuj anisette, zaloguj się na PC, przejdź 2FA i wybierz zespół. Importujesz oryginalną IPA, a Most podpisuje jej kopię. `isideload` używa stabilnego identyfikatora z sufiksem zespołu; pierwsze podpisanie może więc utworzyć inną tożsamość aplikacji niż oryginał. Kolejne odnowienia wymagają tego samego zespołu i oryginału IPA. Nie usuwaj aplikacji, jeśli chcesz zachować jej dane.

Pierwsze wydanie dopuszcza ponowne podpisywanie podstawowych aplikacji bez rozszerzeń. Silnik ma ograniczoną obsługę entitlementów. Aplikacje wymagające niestandardowych capabilities należy dostarczyć już prawidłowo podpisane; nie zakładamy zgodności dowolnej IPA. Most nie odwołuje istniejących certyfikatów po osiągnięciu limitu.

Sukces oznacza zakończone wywołanie usługi instalacyjnej oraz potwierdzony bundle ID i `CFBundleVersion`. Użytkownik powinien następnie uruchomić aplikację na iPhonie. Stan „Sprawdź wynik” oznacza wynik niepewny; nie jest sukcesem i nie wywołuje ślepego ponowienia.

## 4. Własna usługa anisette v3

Program nie zawiera generatora anisette ani nie wybiera automatycznie publicznego operatora. Domyślna konfiguracja kieruje na `http://127.0.0.1:6969`.

Przykładowy projekt: [Dadoum/anisette-v3-server](https://github.com/Dadoum/anisette-v3-server). Przy zainstalowanym Docker Desktop obsługującym kontenery Linux można uruchomić jego obraz **na własnym PC**, zgodnie z instrukcjami autora, ograniczając port do loopback:

```powershell
docker run -d --restart unless-stopped --name most-anisette -p 127.0.0.1:6969:6969 --volume most-anisette-data:/home/Alcoholic/.config/anisette-v3/lib/ dadoum/anisette-v3-server
```

To oddzielna zależność z warunkami jej autorów; Docker Desktop nie jest dołączony do ZIP. Przed użyciem sprawdź aktualny obraz i jego pochodzenie. Własna anisette i logowanie Apple wymagają dostępu do odpowiednich usług Apple.

Na ekranie **Konto Apple** możesz jawnie wybrać inny zaufany adres HTTPS i zaznaczyć zgodę na usługę zewnętrzną. Program nie akceptuje zewnętrznego HTTP ani adresów z osadzonym hasłem. Weryfikacja TLS pozostaje włączona.

## 5. Pierwsza instalacja MostStore

1. Pobierz `MostStore-unsigned.ipa` na PC z tego samego wydania. Jest to rzeczywisty build klienta, bez podpisu dla Twojego konta; sam plik nie instaluje się z Safari.
2. Przygotuj anisette i konto Apple w MostServer.
3. Podłącz iPhone przez USB, zaimportuj IPA MostStore i wybierz podpisanie kontem Apple oraz instalację.
4. Włącz wymagane przez iOS zaufanie/Developer Mode i uruchom MostStore na telefonie.
5. Na PC przejdź do **Połączenie**, podaj prywatny adres IPv4 PC i włącz **MostStore LAN**. Domyślny port: `49241`. Panel lokalny pozostaje osobnym interfejsem na loopback.
6. W **Urządzenia** wybierz **Połącz MostStore** przy właściwym zaufanym telefonie. Pokaże się QR oraz konfiguracja tekstowa.
7. W MostStore otwórz **Połączenie**, zeskanuj kod, potwierdź adres PC i zezwól na dostęp do sieci lokalnej.

Konfiguracja QR zawiera prywatny adres PC, fingerprint certyfikatu, identyfikator urządzenia i klucz dostępu. Jest poświadczeniem, pozostaje ważna do odwołania; nie jest zaproszeniem 120-sekundowym z docelowego projektu Remote. Dostęp można odwołać na PC w ekranie Połączenie. Kod daje dostęp do biblioteki tego użytkownika oraz zlecania zadań wyłącznie na przypisanym urządzeniu. Nie daje dostępu do hasła Apple ani logowania.

Klient sprawdza przypięty certyfikat i standardową politykę TLS z zaufaniem tylko do tego certyfikatu; przekierowania HTTP są odrzucane. Credential jest w Keychain z dostępnością `WhenUnlockedThisDeviceOnly`. Bieżący tryb LAN używa HTTPS i bearer tokena; wzajemne certyfikaty mTLS z docelowego planu Remote nie są jeszcze zaimplementowane.

Po zmianie adresu PC ponownie uruchom LAN i utwórz nowe powiązanie. Serwer tworzy nowy certyfikat dla zmienionego adresu. Udostępnianie portu `49241` do internetu nie jest wspierane. Instalacja z MostStore nadal potrzebuje niezależnego połączenia PC–iPhone przez USB/Wi-Fi, także gdy klient działa na tym samym telefonie.

## 6. Dane i odzyskiwanie

Domyślny katalog Windows: `%LOCALAPPDATA%\MostServer`. Zawiera `most.sqlite3`, oryginały w `artifacts`, katalog roboczy i zaszyfrowany `vault`. W Windows Credential Manager zapisany jest klucz główny `org.mostserver.vault`; bez niego nie da się odczytać vault po przeniesieniu danych. Nie kasuj go jako sposobu wylogowania.

Hasło Apple nie jest utrwalane. Po ponownym uruchomieniu logowanie trzeba wykonać na PC; certyfikaty i stan anisette pozostają zaszyfrowane do ponownego użycia. Baza i oryginały IPA nie są szyfrowane przez Most; podlegają ochronie profilu Windows/dysku.

Po przerwaniu procesu zadania niekońcowe przechodzą do `needs_reconciliation`. Potwierdź wynik na iPhonie i przyciskiem **Sprawdź wynik**. Nie ma gwarancji transakcyjnej instalacji w usługach Apple. Raport diagnostyczny nie zawiera tokenów, adresu konta ani UDID; błędy uwierzytelniania są redagowane.

## 7. Budowanie i testowanie

```sh
cargo test --locked --workspace
cargo run --locked --bin MostServer -- --data-dir /tmp/most-development
```

Do paczki dystrybucyjnej zbierz licencje i wskaż plik `ALL_LICENSES.txt` zmienną `MOST_LICENSES_FILE` podczas `cargo build --locked --release --bin MostServer`; dokładną sekwencję Windows zawiera workflow. Licencje są wbudowane również w pojedynczy EXE i dostępne z panelu oraz przez `--licenses`.

Rust 1.94.0 i dokładne zależności przypina repozytorium. Budowanie EXE wymaga Windows i toolchaina MSVC; nie zmieniaj rozszerzenia binarnego pliku Linux. `scripts/package_windows.py` sprawdza nagłówek PE i uruchamia `--self-test`, zanim tworzy ZIP.

MostStore: na Macu z Xcode uruchom `xcodegen generate` w `apps/ios`, następnie build/archiwizację schematu `MostStore`. Workflow tworzy IPA z `CODE_SIGNING_ALLOWED=NO`; podpis użytkownika następuje dopiero w MostServer.

CLI MostServer: `--port`, `--data-dir`, `--no-browser`, `--self-test`, `--version`. `--self-test` sprawdza bazę i konfigurację, nie urządzenie ani konto Apple.

## 8. Aktualny kontrakt API

| Interfejs | Uwierzytelnianie | Zasoby |
| --- | --- | --- |
| Panel PC `/api/*` | Tylko loopback, kontrola Host/Origin, losowy bearer | Konto/2FA, urządzenia, artefakty, zadania, diagnostyka, zarządzanie klientami |
| MostStore LAN `/v1/*` | HTTPS, pin certyfikatu w iOS, bearer związany z device ID | `GET state`, `POST artifacts`, `POST jobs`, `POST jobs/{id}/reconcile` |

Mobilny snapshot nie zawiera konta Apple i filtruje zadania po urządzeniu. Zlecenie z innym `device_id` jest odrzucane. `Idempotency-Key` klienta jest przestrzenią oddzieloną od panelu PC. Wszystkie pliki przesyłane są jako multipart z polem `ipa`; limit 2 GiB. Pełny przyszły kontrakt MostLink jest w `API.md`; nie opisuje w całości obecnego preview.

## 9. Co potwierdza wydanie

Workflow **Build and release Most** uruchamia testy, kompiluje EXE na Windows i klienta na macOS, następnie publikuje Release oraz manifest pobierania na stronie. Nie wykonuje logowania rzeczywistym kontem ani testu z fizycznym iPhonem. Przejście CI nie potwierdza gotowości Remote, LocalDevVPN lub kompatybilności wszystkich IPA/iOS.

Zbudowana paczka nie pojawi się na stronie po nieudanym CI. Jeśli konkretna wersja została już wydana z innego commita, należy zwiększyć wersję aplikacji przed nowym wydaniem. Ponowienie tego samego uruchomienia może dokończyć publikację tego samego commita.

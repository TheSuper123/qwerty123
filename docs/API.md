# Most Protocol v1 — projekt kontraktu

To specyfikacja planowanego API, nie działający backend. Kontrakt zostanie zamrożony po prototypie transportu; wtedy powstanie OpenAPI i wspólne fixtures testowe.

## 1. Reguły wspólne

- API pod `/v1`, JSON UTF-8, czasy RFC 3339 w UTC; interfejs przelicza strefę użytkownika.
- Poza parowaniem każde żądanie sieciowe wymaga mTLS i autoryzacji konkretnego urządzenia.
- Prywatne dane urządzeń i konta nie pojawiają się w publicznym healthchecku. Zwykły panel PC korzysta z lokalnego IPC z kontrolą uprawnień systemu.
- Zmiany stanu otrzymują `Idempotency-Key` i identyfikator żądania. Klucz jest związany z klientem, endpointem i skrótem treści; inne dane z tym samym kluczem dają `409`.
- Limit JSON: 256 KiB. Pliki wysyłane osobnym strumieniem z zadeklarowanym rozmiarem, SHA-256 i limitem artefaktu.
- Obiekty mają niezgadywalne identyfikatory; znajomość ID nie zastępuje sprawdzenia właściciela.
- Serwer ignoruje jedynie udokumentowane dodatkowe pola zgodnej wersji; niezgodny major protokołu odrzuca przed otwarciem kanału danych.

## 2. Parowanie

| Operacja | Dostęp | Rezultat |
| --- | --- | --- |
| Utworzenie zaproszenia | Wyłącznie lokalny interfejs PC | Jednorazowy QR i krótki termin ważności |
| `POST /v1/pairings/redeem` | TLS z pinem PC + token QR | Żądanie oczekujące na lokalne potwierdzenie i sekret pobrania wyniku |
| `POST /v1/pairings/{id}/complete` | TLS z pinem + jednorazowy sekret pobrania | Po potwierdzeniu: certyfikat dla publicznego klucza klienta |
| `DELETE /v1/clients/{id}` | Lokalny PC lub klient odwołujący własny dostęp | Natychmiastowe odwołanie i zamknięcie sesji |

Sekrety bootstrapu trafiają do body lub chronionego nagłówka, nigdy do query stringu lub logów. Próby są ograniczone, np. 5 na minutę na zaproszenie; po wygaśnięciu zaproszenie jest bezużyteczne. Potwierdzenie na PC ma być związane z dokładnym kluczem klienta, nie samą nazwą telefonu.

## 3. Kontrola i artefakty

| Metoda i ścieżka | Działanie |
| --- | --- |
| `GET /v1/server` | Wersja, możliwości protokołu i stan własnego serwera po uwierzytelnieniu |
| `GET /v1/devices` | Wyłącznie urządzenia przypisane klientowi |
| `GET /v1/devices/{id}/capabilities` | USB/LAN/Remote, data ostatniego testu, powód niedostępności |
| `GET /v1/devices/{id}/apps` | Stan aplikacji uzgodniony z urządzeniem albo jawnie oznaczony cache |
| `POST /v1/artifacts` | Rezerwacja uploadu: rozmiar, nazwa, oczekiwany hash |
| `PUT /v1/artifacts/{id}/content` | Strumień IPA; niewłaściwa długość/hash usuwa niepełny wynik |
| `GET /v1/artifacts/{id}` | Analiza pakietu i zgodność z urządzeniem/kontem |
| `POST /v1/jobs` | Zlecenie instalacji, odnowienia lub diagnozy |
| `GET /v1/jobs/{id}` | Stan, bieżący etap, wynik i kod błędu |
| `POST /v1/jobs/{id}/cancel` | Anulowanie, jeżeli etap na to pozwala |
| `GET /v1/events?after={sequence}` | SSE z wznowieniem od ostatniego numeru zdarzenia |
| `GET /v1/diagnostics` | Raport pozbawiony sekretów, eksportowany świadomie przez użytkownika |

W MVP upload po zerwaniu rozpoczyna się od nowa, a nie udaje częściowego wznowienia. W Beta Remote można dodać wersjonowany protokół chunków ze skrótami; wymaga osobnych testów.

Przykład zlecenia:

```http
POST /v1/jobs
Idempotency-Key: example-install-001
Content-Type: application/json
```

```json
{
  "type": "install",
  "device_id": "dev_example",
  "artifact_id": "ipa_example",
  "transport": "auto",
  "allow_cellular": false
}
```

`202 Accepted` oznacza wyłącznie przyjęcie zadania. Przykładowa odpowiedź:

```json
{
  "job_id": "job_example",
  "state": "queued",
  "events_after": 42
}
```

`auto` preferuje USB, potem LAN; Remote wymaga wcześniejszej zgody użytkownika i pozytywnego testu możliwości. Odnowienie wskazuje `installed_app_id`; serwer ustala powiązany oryginał i stabilne mapowanie bundle ID. Logowanie Apple oraz 2FA nie są częścią tego mobilnego API.

## 4. Stany zadań

```text
queued → validating → provisioning → signing → awaiting_transport
       → transferring → installing → verifying → succeeded

Oczekiwanie: waiting_for_pc_action / waiting_for_device / paused
Końcowe: succeeded / failed / cancelled
Po utracie pewnego wyniku: needs_reconciliation
```

To kolejne etapy jednej operacji; przejście z `awaiting_transport` nie pomija podpisywania. Każda zmiana ma trwały numer zdarzenia. UI nie interpretuje zamknięcia socketu jako sukcesu ani porażki instalacji.

Anulowanie podczas provisioningu może pozostawić już utworzone App ID/profil w Apple; wynik operacji pokazuje skutki. Anulowanie nie odwołuje automatycznie certyfikatów. Po wysłaniu polecenia instalacji odpowiedź na anulowanie może wynosić `409 cancellation_not_safe`; najpierw należy uzgodnić wynik z iOS.

Idempotency key jest przechowywany co najmniej 7 dni. Po restarcie `installing`/`verifying` wymaga uzgodnienia stanu zamiast ślepego powtarzania. Przy niedostępności urządzenia stan pozostaje niepewny.

## 5. Zdarzenia

SSE służy do stanu UI; WSS w kolejnym rozdziale służy do mostu urządzenia. Zdarzenia SSE mają `id`, `event` i JSON `data`:

```text
id: 43
event: job.progress
data: {"job_id":"job_example","stage":"transferring","bytes":5242880,"total":10485760}
```

Typy: `job.state`, `job.progress`, `device.connection`, `profile.expiry`, `server.action_required`. Replay jest ograniczony, np. 24 godziny; klient ze zbyt starym numerem dostaje `410 event_history_expired` i pobiera snapshot stanu.

## 6. MostLink — kanał danych Remote

`GET /v1/link` przechodzi do WSS po weryfikacji certyfikatu, przypisania urządzenia i aktywnego zadania. Zewnętrzny LocalDevVPN nie zwalnia z mTLS.

1. `HELLO`: wersja, sesja, device ID i sprawdzone możliwości adaptera. Kompatybilność wersji jest dwustronna.
2. `OPEN`: serwer inicjuje strumień przypisany do zadania. Dozwolone są lockdown i usługi uzyskane z zatwierdzonej sesji; nie ma dowolnego hostname/port przekazanego przez klienta API.
3. `DATA`: bajty w ramkach binarnych. Propozycja nagłówka v1: typ `u8`, stream ID `u32`, numer `u64`, długość `u32`, big-endian; payload najwyżej 64 KiB.
4. `WINDOW_UPDATE`: kredyt przepływu; najwyżej 8 strumieni i 8 MiB łącznego bufora na sesję. Przekroczenie limitu kończy sesję kontrolowanym błędem.
5. `HALF_CLOSE`, `CLOSE`, `ERROR`: jawne kończenie strumieni z potwierdzeniem. `PING/PONG` wykrywa brak łączności, ale nie potwierdza działania instalatora.

Te liczby są początkowymi limitami projektu, do pomiaru w prototypie. WebSocket jest uporządkowanym kanałem; numeracja służy walidacji i diagnostyce, nie odtwarzaniu w ciemno sesji Apple.

Po ponownym połączeniu tworzymy nową sesję i nowe kanały usług. Nie wznawiamy starego, zaszyfrowanego strumienia lockdown od dowolnego bajtu. Uzgadniamy stan zadania, sprawdzamy staging artefaktu i dopiero wtedy wybieramy bezpieczne ponowienie.

Proces iOS może być zawieszony. MVP Remote działa przy aktywnym ekranie klienta; serwer stosuje heartbeat i timeout bez zakładania stałego działania aplikacji w tle. Dozwolone lokalne adresy i dynamiczne porty wynikają z aktywnej sesji własnego urządzenia i są walidowane po obu stronach.

## 7. Kody błędów i działanie użytkownika

| Kod | Znaczenie / następny krok |
| --- | --- |
| `pc_unreachable` | Brak drogi do PC; sprawdź zasilanie, adres i trasę. To zwykle błąd klienta połączenia, nie odpowiedź nieosiągalnego serwera. |
| `device_not_trusted` | Potrzebne parowanie USB i potwierdzenie na iPhonie |
| `developer_mode_required` | Włącz wymagany przez system tryb i ponów sprawdzenie |
| `apple_auth_required` | Zaloguj się ponownie na PC |
| `apple_two_factor_required` | Dokończ 2FA na PC; kod nie jest zbierany przez klienta mobilnego |
| `account_limit_reached` | Pokaż informację uzyskaną z profilu/usługi; nie zgaduj resetu limitu |
| `entitlement_unsupported` | Wskazana capability jest niedostępna dla wybranego profilu |
| `ipa_encrypted` | Pakiet chroniony DRM nie jest obsługiwany |
| `ipa_invalid` / `artifact_hash_mismatch` | Odrzuć pakiet bez rozpoczynania podpisywania |
| `local_tunnel_unavailable` | Test lokalnego mostu się nie powiódł; pokaż instrukcję LocalDevVPN |
| `remote_transport_unsupported` | Dana konfiguracja nie przeszła bramki Remote |
| `device_locked` | Odblokuj urządzenie i sprawdź ponownie dostępność usług |
| `installation_outcome_unknown` | Uzgodnij listę aplikacji; nie pokazuj sukcesu i nie powielaj instalacji |
| `self_refresh_requires_pc_transport` | Odnowienie MostStore przez USB/LAN, dopóki samoodnowienie Remote nie jest potwierdzone |

Błąd ma strukturę `code`, `message`, `retryable`, `request_id` i opcjonalne niesekretne `details`. Statusy HTTP: 400 dla błędnego kontraktu, 403 dla braku uprawnień, 404 dla niedostępnego obiektu, 409 dla konfliktu, 413 dla rozmiaru, 422 dla nieobsługiwanego pakietu i 429 dla limitów. Nieprawidłowy certyfikat może zakończyć połączenie jeszcze przed HTTP.

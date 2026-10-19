# Roadmapa, testy i wydanie

Dokument obejmuje planowanie i implementację. Obecnie zrealizowane są dokumentacja oraz statyczna strona; żaden kamień milowy urządzenia nie jest jeszcze zaliczony.

## 1. Etapy i bramki

| Etap | Praca | Dowód ukończenia | Szacunek zespołu |
| --- | --- | --- | --- |
| 0 · wykonalność | Windows, sterowniki, signer, logowanie i profile, prototyp lokalnego adaptera | Własna IPA uruchomiona po instalacji z PC; oddzielny raport dla konta free i paid, wyniki prób Remote | 2–3 tygodnie |
| 1 · USB MVP | Kolejka, magazyn IPA, GUI PC, bootstrap MostStore, parowanie mTLS, odnowienie USB | 10 kolejnych instalacji/odnowień na urządzeniu; kontrola wersji, profili i danych | 3–4 tygodnie |
| 2 · LAN | Wykrywanie, Wi-Fi, reguły zapory, terminy i przypomnienia, diagnostyka | Kabel odłączony, ponowna instalacja/odnowienie w tej samej sieci, test izolacji klientów | 2–3 tygodnie |
| 3 · Remote + LocalDevVPN | MostLink, relay, lokalny adapter, kreator, obsługa przerwań | Fizyczny telefon poza LAN, działające LTE/5G i Wi-Fi zewnętrzne; pozytywna bramka z architektury | 3–5 tygodni po pozytywnym R&D |
| 4 · beta / 1.0 | Pakowanie, aktualizacje, testy awarii, dostępność, dokumentacja i audyt zależności | Odtwarzalny build, brak krytycznych problemów, pełny test akceptacyjny | 3–5 tygodni |

Razem orientacyjnie **16–24 tygodnie z buforem** dla dwóch osób znających Windows oraz Swift/iOS. Dla jednej osoby pracującej częściowo: orientacyjnie **6–12 miesięcy**. To plan prac, nie obietnica terminu. Brak zgodnej autoryzacji Apple, problem z uprawnieniami VPN lub nieudana bramka Remote może wydłużyć R&D bez górnej granicy.

### Etap 0 — zadania, które blokują dalsze obietnice

- Zanotować dokładny Windows build, iOS build, model telefonu, sterowniki i wersje narzędzi.
- Zbudować albo dobrać zgodny silnik podpisujący Windows; przeanalizować licencję i dystrybucję.
- Udowodnić lokalną obsługę wymagań uwierzytelniania/anisette bez nieznanej publicznej usługi.
- Zainstalować małą własną aplikację przez USB; sprawdzić profil i ręcznie ją uruchomić.
- Osobno przejść scenariusz bezpłatnego Apple Account z 2FA. Sukces płatnego profilu nie zamyka tego zadania.
- Sprawdzić zewnętrzny LocalDevVPN, lokalny endpoint urządzenia i czy WSS do PC działa przy aktywnym tunelu.
- Przeprowadzić minimalną instalację przez kontrolowany relay; ustalić, gdzie kończą się sesje TLS usług Apple.
- Zmierzyć zachowanie przy aktualizacji aplikacji będącej relayem i zapisać ograniczenie samoodnowienia.

Każdy wynik ma status `passed`, `failed`, `blocked` albo `not_run`, log bez sekretów i procedurę odtworzenia. Nie zaliczamy nieprzetestowanego elementu na podstawie deklaracji biblioteki.

## 2. Backlog według priorytetu

| Priorytet | Zadanie | Zależność |
| --- | --- | --- |
| P0 | Device Adapter i katalog wspieranych konfiguracji | Bramka 0 |
| P0 | Signer, zespół, profile, bezpieczna sesja Apple | Bramka 0 i wybór licencji |
| P0 | Bootstrap klienta i stałe mapowanie bundle ID | Signer + USB |
| P0 | Walidacja IPA, magazyn, kolejka i uzgadnianie wyniku | Model zagrożeń i adapter |
| P0 | MostStore: Moje aplikacje, import, postęp, błędy | Kontrakt API |
| P1 | LAN i test bez kabla | USB MVP |
| P1 | MostLink i lokalny adapter LocalDevVPN | Udany prototyp Remote |
| P1 | Samoodnowienie Remote albo jawna blokada z przyciskiem instrukcji PC | Udowodnione przekazanie instalacji systemowi |
| P1 | Kreator Remote i diagnostyka CGNAT/VPN | MostLink |
| P1 | Instalator Windows, podpis wydawcy, bezpieczna aktualizacja | Audyt dystrybucji |
| P2 | Źródła aplikacji, filtrowanie i kontrola aktualizacji | Stabilny import pliku |
| P2 | Własne rozszerzenie VPN | Uprawnienia i dystrybucja Apple |
| P2 | Serwer macOS, wiele urządzeń, dodatkowe usługi deweloperskie | Osobny zakres i macierz testów |

## 3. Testy automatyczne i fizyczne

**CI na każdym PR:** walidacja kontraktów, parser IPA na celowo uszkodzonych archiwach, autoryzacja per-device, idempotencja, maszyna stanów, redakcja sekretów, testy ramek transportu, zależności i kontrola linków strony. Mocki obejmują tylko kontrolowane scenariusze; nie dowodzą kompatybilności z Apple.

**Buildy:** Windows dla serwera i helperów; macOS/Xcode dla aplikacji i bibliotek iOS. Narzędzia są przypięte, release jest odtwarzalny, artefakty mają pochodzenie i wykaz zależności. Sekrety podpisujące nie trafiają do workflow uruchamianego z niezaufanych forków.

**Laboratorium z urządzeniem:** instalacja, odnowienie, uruchomienie, profile, USB/LAN/Remote. Testy wymagające Apple i fizycznego iPhone’a są osobnymi zadaniami ręcznymi lub HIL; zielony zwykły CI ich nie zastępuje. Publiczne workflow strony nie ma dostępu do tych sekretów.

### Minimalna macierz

| Wymiar | Warianty |
| --- | --- |
| Windows | Windows 11 x64, czysta instalacja i aktualizacja poprzedniej wersji Most |
| iOS | 17.4 jako kandydat minimum oraz aktualna stabilna wersja; dokładne buildy zapisane w raporcie |
| Konto | Free Personal Team, płatne developer; odrębne wyniki i ograniczenia |
| Transport | USB, LAN bez kabla, obce Wi-Fi, LTE/5G, PC za CGNAT bez trasy |
| Pakiet | Mała własna IPA, większy pakiet, app extension, niezgodne entitlementy, uszkodzony ZIP, DRM |
| Awaria | Uśpiony PC, restart PC, przerwane Wi-Fi, zablokowany telefon, ubity klient, wygaśnięta sesja Apple |
| Zaufanie | Nieznany certyfikat, odwołany klient, zły device ID, przeterminowany QR, powtórzone żądanie |
| VPN | Tunel wyłączony, zewnętrzny LocalDevVPN, konflikt z VPN do domu, próba własnego providera tylko gdy dopuszczona |

### Test akceptacyjny całej wizji

1. Czysty Windows i fizyczny iPhone; bootstrap z instrukcją, bez terminala użytkownika.
2. MostStore i własna testowa aplikacja mają poprawne profile i uruchamiają się.
3. Zapisana dana w aplikacji przetrwa zwykłe odnowienie z niezmienioną tożsamością.
4. Kabel odłączony: ta sama operacja działa w LAN.
5. Telefon poza domem: udana instalacja przez MostLink i lokalny most, bez bezpośredniego połączenia USB/LAN do PC.
6. Przerwanie transferu i restart PC nie tworzą fałszywego sukcesu ani podwójnego zadania.
7. Cofnięty certyfikat klienta uniemożliwia następne połączenie.
8. Samoodnowienie klienta ma osobny udokumentowany wynik. Jeśli nie działa Remote, UI kieruje do USB/LAN, a opis wydania wskazuje ograniczenie.
9. Eksport raportu nie ujawnia kluczy, danych Apple ani rekordu parowania.

## 4. Ryzyka i reakcje

| Ryzyko | Reakcja |
| --- | --- |
| Apple zmienia autoryzację lub ogranicza konto | Mały adapter, jawne błędy, ponowne logowanie na PC; brak obietnicy wiecznej zgodności |
| Brak poprawnego signera Windows lub lokalnej anisette | Zatrzymać bramkę 0; nie sprzedawać jako rozwiązane przez sam import `.p12` |
| Lokalny kanał nie działa na danym iOS | Oznaczyć konfigurację niewspieraną, zachować USB/LAN, zebrać wyniki R&D |
| MostStore jest zabijany podczas samoodnowienia | Odrębny test dokończenia przez iOS; do tego czasu PC USB/LAN |
| CGNAT i konflikt dwóch VPN | Kreator wykrywa brak trasy, nie włącza Remote pozornie; własna osiągalna trasa lub dalszy etap relaya |
| Brak uprawnień własnego VPN | Zewnętrzny LocalDevVPN jako wariant bazowy, bez obietnicy pojedynczej IPA dla darmowego konta |
| Niezgodna licencja zależności | Audyt przed kopiowaniem/linkowaniem i dystrybucją; wybór zgodnej licencji lub innego komponentu |
| Złośliwa IPA / źródło | Ograniczone pobieranie i rozpakowanie, jawna zgoda, izolacja workerów |
| Zawieszenie iOS w tle / wygaśnięcie aplikacji | Powiadomienia, ręczne odnowienie, sprawdzony ratunek przez kabel |

## 5. Zasoby i koszty

Potrzebne: Windows PC z uprawnieniami do instalacji sterowników, co najmniej jeden fizyczny iPhone, kabel, testowe konta Apple, dostęp do Maca/Xcode do budowania klienta i druga sieć/LTE do Remote. Dwa iPhone’y na różnych buildach istotnie zmniejszają ryzyko fałszywej zgodności.

Oprogramowanie nie wymaga płatnego hostingu backendu Most. Mogą wystąpić koszty Apple Developer Program, sprzętu/Maca lub runnera macOS, podpisu instalatora Windows, własnego DNS i transferu. Ceny należy sprawdzić dla regionu w chwili zakupu. Płatne konto nie jest warunkiem MVP dla użytkownika, jeśli bramka konta free zostanie zaliczona; własny provider VPN może mieć inne wymagania dystrybucji.

Nazwa/licencja produktu wymagają decyzji przed dystrybucją. Rekomendacja robocza: otwarte źródła z licencją zgodną z faktycznie użytymi komponentami GPL/AGPL. Ten dokument sam nie nadaje licencji całemu repozytorium.

## 6. Strona i komunikacja wydania

Strona projektu już pokazuje koncepcję, tryby i etapy; nie oferuje fikcyjnego instalatora. GitHub Pages hostuje tylko statyczne materiały, nigdy logowania Apple, IPA użytkowników, sekretów lub serwera instalacyjnego.

Publikacja strony: workflow z katalogu `.github/workflows`, artefakt wyłącznie `site/`, źródło Pages ustawione na GitHub Actions. Po wdrożeniu należy sprawdzić adres projektu, CSS/JS w podkatalogu, widok mobilny i odnośniki dokumentacji. Wydanie MostServer/MostStore w przyszłości dostanie osobne workflow i podpisane artefakty.

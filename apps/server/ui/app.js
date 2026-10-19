"use strict";

const $ = (selector) => document.querySelector(selector);
const tokenFromUrl = new URLSearchParams(location.hash.slice(1)).get("token");
if (tokenFromUrl && /^[a-f0-9]{64}$/.test(tokenFromUrl)) sessionStorage.setItem("most-access-token", tokenFromUrl);
if (location.hash.startsWith("#token=")) history.replaceState(null, "", location.pathname);
let token = sessionStorage.getItem("most-access-token");
let snapshot = null;
let deviceReport = null;
let accountRenderKey = "";
let jobsRenderKey = "";
let artifactRenderKey = "";
let uploading = false;
let selectedArtifact = null;
let requestKey = null;
let lastJobBody = null;
let refreshing = false;
let toastTimer;
let networkRenderKey = "";
const titles = {overview:"Twój pulpit",library:"Biblioteka IPA",devices:"Twoje urządzenia",account:"Konto Apple",connection:"Połączenie",diagnostics:"Diagnostyka"};

function element(tag, className, content) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (content !== undefined) el.textContent = content;
  return el;
}
function button(label, className, handler) {
  const el = element("button", className, label);
  el.type = "button";
  el.addEventListener("click", handler);
  return el;
}
function toast(message, error = false) {
  const el = $("#toast"); el.textContent = message; el.className = error ? "toast error" : "toast"; el.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, error ? 12000 : 6000);
}
function bytes(value) { return new Intl.NumberFormat("pl-PL", {maximumFractionDigits:1}).format(value / (1024 * 1024)) + " MiB"; }
function date(value) { const parsed = new Date(value); return Number.isNaN(parsed.getTime()) ? "Nieznana data" : parsed.toLocaleString("pl-PL"); }
function showScreen(name) {
  if (!titles[name]) return;
  document.querySelectorAll("[data-page]").forEach(page => { page.hidden = page.dataset.page !== name; });
  document.querySelectorAll("nav [data-screen]").forEach(tab => { if (tab.dataset.screen === name) tab.setAttribute("aria-current","page"); else tab.removeAttribute("aria-current"); });
  $("#screen-title").textContent = titles[name];
  if (name === "connection" && token) loadClients();
  window.scrollTo({top:0, behavior:"instant"});
}
document.querySelectorAll("[data-screen]").forEach(tab => tab.addEventListener("click", () => showScreen(tab.dataset.screen)));
$(".brand[href]").addEventListener("click", event => { event.preventDefault(); showScreen("overview"); });

async function api(path, options = {}) {
  const headers = new Headers(options.headers);
  headers.set("Authorization", "Bearer " + token);
  if (options.body && typeof options.body === "string") headers.set("Content-Type", "application/json");
  const response = await fetch(path, {...options, headers, cache:"no-store"});
  if (response.status === 401) { token = null; sessionStorage.removeItem("most-access-token"); showUnlock(); throw new Error("Otwórz ponownie panel z odnośnika w oknie MostServer."); }
  if (!response.ok) {
    const failure = await response.json().catch(() => null);
    throw new Error(failure?.message || "Operacja nie powiodła się (HTTP " + response.status + ").");
  }
  return response.status === 204 ? null : response.json();
}
async function runAction(action, success) {
  try { await action(); if (success) toast(success); await refresh(); }
  catch (error) { toast(error.message, true); }
}
function showUnlock() { $("#unlock").hidden = false; $("#shell").hidden = true; }
$("#unlock-form").addEventListener("submit", async event => {
  event.preventDefault(); token = $("#access-token").value.trim(); $("#access-token").value = "";
  if (!/^[a-f0-9]{64}$/.test(token)) { toast("Klucz lokalny ma 64 znaki szesnastkowe.",true); return; }
  sessionStorage.setItem("most-access-token",token); await refresh();
});

async function refresh() {
  if (!token || refreshing) return;
  refreshing = true;
  try {
    snapshot = await api("/api/state");
    $("#shell").hidden = false; $("#unlock").hidden = true; $("#offline").hidden = true;
    $("#version").textContent = "v" + snapshot.version + " · preview";
    $("#footer-version").textContent = snapshot.version;
    $("#stat-artifacts").textContent = String(snapshot.artifacts.length);
    $("#stat-success").textContent = String(snapshot.jobs.filter(job => job.state === "succeeded").length);
    $("#operation-state").textContent = snapshot.busy ? "Operacja w toku" : "Gotowy";
    $("#operation-state").classList.toggle("busy",snapshot.busy);
    $("#shutdown").disabled = snapshot.busy;
    const networkKey = JSON.stringify(snapshot.network);
    if (networkRenderKey !== networkKey) {
      networkRenderKey = networkKey;
      const network = snapshot.network;
      $("#lan-state").textContent = network.enabled ? "Aktywne: " + network.url + " · certyfikat " + network.certificate_sha256.slice(0,16) + "…" : (network.message || "Wyłączone. Domyślnie żaden serwer sieciowy nie jest udostępniany.");
      $("#start-lan").disabled = network.enabled;
      $("#lan-address").disabled = network.enabled;
      if (deviceReport) renderDevices();
    }
    const artifactKey = JSON.stringify(snapshot.artifacts);
    if (artifactRenderKey !== artifactKey) { artifactRenderKey = artifactKey; renderArtifacts(); }
    const jobsKey = JSON.stringify(snapshot.jobs);
    if (jobsRenderKey !== jobsKey) { jobsRenderKey = jobsKey; renderJobs(); }
    const accountKey = JSON.stringify(snapshot.account);
    if (accountRenderKey !== accountKey) { accountRenderKey = accountKey; renderAccount(); }
  } catch (error) {
    $("#offline").hidden = false;
    if (!snapshot && token) toast("Nie można otworzyć panelu. Sprawdź okno MostServer i lokalny klucz dostępu.",true);
  } finally { refreshing = false; }
}

function renderArtifacts() {
  const list = $("#artifact-list"); list.replaceChildren();
  if (!snapshot.artifacts.length) { list.append(element("div","empty","Biblioteka jest pusta. Dodaj swoją pierwszą IPA.")); return; }
  for (const artifact of snapshot.artifacts) {
    const metadata = artifact.metadata;
    const card = element("article","card artifact-card");
    const head = element("div","artifact-header"); head.append(element("span","app-icon",metadata.name.slice(0,1).toUpperCase()),element("h3",null,metadata.name));
    const details = element("div","metadata");
    details.append(element("span",null,"v" + metadata.version),element("span",null,"build " + metadata.build),element("span",null,bytes(artifact.size)));
    if (metadata.minimum_ios) details.append(element("span",null,"iOS " + metadata.minimum_ios + "+"));
    card.append(head,element("p","bundle",metadata.bundle_id),details);
    if (metadata.profile) {
      const expired = metadata.profile.expired || (metadata.profile.expires_at && new Date(metadata.profile.expires_at) <= new Date());
      card.append(element("p",expired ? "profile error-text" : "profile",(expired ? "Profil wygasł" : "Profil: " + (metadata.profile.expires_at ? date(metadata.profile.expires_at) : "brak daty")) + " · metadane, bez potwierdzenia podpisu"));
    } else card.append(element("p","profile muted","Brak czytelnego profilu · wymagane podpisanie"));
    if (metadata.extensions) card.append(element("p","profile muted","Rozszerzenia: " + metadata.extensions + " · instalacja wcześniej podpisanej IPA"));
    card.append(button("Zainstaluj na urządzeniu ↗","primary",() => openInstall(artifact)));
    list.append(card);
  }
}

function renderJobs() {
  const list = $("#job-list"); list.replaceChildren();
  if (!snapshot.jobs.length) { list.append(element("div","empty","Nie było jeszcze żadnych instalacji.")); return; }
  const labels = {queued:"W kolejce",validating:"Sprawdzanie",provisioning:"Przygotowywanie profilu",signing:"Podpisywanie",installing:"Instalacja",verifying:"Potwierdzanie",succeeded:"Potwierdzono instalację",failed:"Operacja nieudana",needs_reconciliation:"Sprawdź wynik",cancelled:"Anulowano"};
  for (const job of snapshot.jobs) {
    const artifact = snapshot.artifacts.find(item => item.id === job.artifact_id);
    const row = element("article","job-row " + job.state);
    const symbol = job.state === "succeeded" ? "✓" : job.state === "failed" ? "!" : job.state === "needs_reconciliation" ? "?" : "↗";
    row.append(element("span","job-symbol",symbol));
    const info = element("div"); info.append(element("h3",null,(artifact?.metadata.name || "Aplikacja") + " · " + (labels[job.state] || job.state)),element("p",null,job.message));
    if (job.progress !== null && job.state === "installing") { const progress = element("progress"); progress.max = 100; progress.value = job.progress; progress.setAttribute("aria-label","Postęp transferu i instalacji"); info.append(progress); }
    if (job.state === "needs_reconciliation") {
      const check = button("Sprawdź wynik na urządzeniu","secondary",() => runAction(() => api("/api/jobs/" + encodeURIComponent(job.id) + "/reconcile",{method:"POST"}),"Wynik potwierdzony."));
      check.disabled = !job.effective_bundle_id; if (check.disabled) check.title = "Przed ponowieniem sprawdź aplikacje na iPhonie; brak zapisanego ID rozpoczętej instalacji.";
      info.append(check);
    }
    const time = element("time",null,date(job.updated_at)); time.dateTime = job.updated_at; row.append(info,time); list.append(row);
  }
}

async function scanDevices() {
  const control = $("#scan-devices"); control.disabled = true; control.textContent = "Sprawdzanie…";
  try {
    deviceReport = await api("/api/devices"); renderDevices();
  } catch (error) { toast(error.message,true); }
  finally { control.disabled = false; control.textContent = "Sprawdź urządzenia"; }
}
$("#scan-devices").addEventListener("click",scanDevices);
function renderDevices() {
  const list = $("#device-list"); list.replaceChildren();
  $("#stat-devices").textContent = String(deviceReport.devices.length);
  $("#stat-device-note").textContent = deviceReport.available ? "ostatni wynik sprawdzania" : "sprawdź sterowniki Apple";
  const notice = $("#device-notice");
  notice.textContent = deviceReport.message || (deviceReport.devices.length ? "Wybierz urządzenie. iOS może wymagać odblokowania i aktywnego Developer Mode." : "Usługa Apple odpowiada, ale nie wykryła urządzenia. Podłącz iPhone przez USB.");
  notice.classList.toggle("danger",!deviceReport.available);
  for (const device of deviceReport.devices) {
    const card = element("article","card device-card"); card.append(element("span","tag",device.connection + " · " + (device.trusted ? "Zaufane" : "Wymaga zaufania")),element("h3",null,device.name));
    card.append(element("p",null,(device.ios ? "iOS " + device.ios + " · " : "") + "ID …" + device.id.slice(-8)));
    const actions = element("div","actions");
    if (!device.trusted) actions.append(button("Sparuj / Zaufaj","primary",async (event) => {
      const control = event.currentTarget; control.disabled = true; control.textContent = "Potwierdź Zaufaj na telefonie…";
      await runAction(() => api("/api/devices/" + encodeURIComponent(device.id) + "/pair",{method:"POST"}),"Urządzenie sparowane."); await scanDevices();
    }));
    const apps = button("Pokaż aplikacje","secondary",() => loadApps(device)); apps.disabled = !device.trusted; actions.append(apps); card.append(actions); list.append(card);
    if (device.trusted) actions.append(button("Połącz MostStore","secondary",() => createPairing(device)));
  }
}
async function loadApps(device) {
  try {
    const apps = await api("/api/devices/" + encodeURIComponent(device.id) + "/apps");
    $("#installed-section").hidden = false; $("#installed-heading").textContent = "Aplikacje · " + device.name;
    const area = $("#installed-apps"); area.replaceChildren();
    if (!apps.length) { area.append(element("p","muted","Brak aplikacji użytkownika w odpowiedzi urządzenia.")); return; }
    const table = element("table","app-table"); const header = element("thead"); const heading = element("tr");
    ["Aplikacja","Wersja","Build"].forEach(label => { const th = element("th",null,label); th.scope = "col"; heading.append(th); }); header.append(heading); table.append(header);
    const body = element("tbody"); for (const app of apps) { const row = element("tr"); row.append(element("td",null,app.name + " · " + app.bundle_id),element("td",null,app.version),element("td",null,app.build)); body.append(row); } table.append(body); area.append(table);
  } catch (error) { toast(error.message,true); }
}

const fileInput = $("#ipa-file");
for (const id of ["import-top","start-import","step-import","library-import","drop-import"]) $("#" + id).addEventListener("click",() => { if (!uploading) fileInput.click(); });
fileInput.addEventListener("change",() => { if (fileInput.files[0]) importIpa(fileInput.files[0]); fileInput.value = ""; });
const drop = $("#drop-zone");
for (const type of ["dragenter","dragover"]) drop.addEventListener(type,event => { event.preventDefault(); drop.classList.add("dragging"); });
for (const type of ["dragleave","drop"]) drop.addEventListener(type,event => { event.preventDefault(); drop.classList.remove("dragging"); });
drop.addEventListener("drop",event => { if (event.dataTransfer.files.length === 1) importIpa(event.dataTransfer.files[0]); else toast("Wybierz jeden plik IPA naraz.",true); });
function importIpa(file) {
  if (uploading) return;
  if (!file.name.toLowerCase().endsWith(".ipa") || file.size > 2 * 1024 ** 3) { toast("Wybierz plik .ipa o rozmiarze do 2 GiB.",true); return; }
  showScreen("library"); uploading = true; $("#upload-state").hidden = false; $("#upload-message").textContent = "Przesyłanie pliku na lokalny serwer…"; $("#upload-progress").value = 0;
  const body = new FormData(); body.append("ipa",file);
  const xhr = new XMLHttpRequest(); xhr.open("POST","/api/artifacts"); xhr.setRequestHeader("Authorization","Bearer " + token); xhr.timeout = 600000;
  xhr.upload.addEventListener("progress",event => { if (event.lengthComputable) $("#upload-progress").value = event.loaded / event.total * 100; });
  xhr.upload.addEventListener("load",() => { $("#upload-message").textContent = "Sprawdzanie archiwum, integralności i metadanych…"; $("#upload-progress").removeAttribute("value"); });
  const finish = async () => { uploading = false; $("#upload-state").hidden = true; await refresh(); };
  xhr.addEventListener("load",async () => {
    let response; try { response = JSON.parse(xhr.responseText); } catch { response = null; }
    if (xhr.status >= 200 && xhr.status < 300) toast("IPA została dodana do biblioteki.");
    else toast(response?.message || "Nie udało się zaimportować IPA.",true);
    await finish();
  });
  for (const type of ["error","timeout","abort"]) xhr.addEventListener(type,async () => { toast("Import przerwany. Wybierz plik ponownie.",true); await finish(); });
  xhr.send(body);
}

async function openInstall(artifact) {
  if (!snapshot) return;
  if (snapshot.busy) { toast("Poczekaj na zakończenie bieżącej operacji.",true); return; }
  if (!deviceReport) await scanDevices();
  selectedArtifact = artifact; requestKey = null; lastJobBody = null;
  $("#install-app-name").textContent = artifact.metadata.name + " · " + artifact.metadata.bundle_id;
  const select = $("#install-device"); select.replaceChildren();
  for (const device of deviceReport?.devices || []) if (device.trusted) { const option = element("option",null,device.name + " · " + device.connection); option.value = device.id; select.append(option); }
  if (!select.options.length) { const option = element("option",null,"Najpierw połącz zaufane urządzenie"); option.value = ""; select.append(option); }
  $("#install-mode").value = artifact.metadata.profile && artifact.metadata.has_signature_files ? "presigned" : "resign";
  $("#install-error").textContent = ""; installExplanation(); $("#install-dialog").showModal();
}
function installExplanation() {
  const resign = $("#install-mode").value === "resign";
  const loggedIn = snapshot?.account.status === "logged_in" && snapshot.account.selected_team;
  $("#install-explanation").textContent = resign ? "Podpis zostanie przygotowany na PC. Wybrany zespół Apple może zmienić identyfikator na stałe mapowanie używane przez silnik podpisujący. Ta wersja obsługuje podstawowe aplikacje bez rozszerzeń." : "iOS sprawdzi podpis i zgodność profilu z urządzeniem. Obecność metadanych profilu w bibliotece nie potwierdza ważności kryptograficznej podpisu.";
  const unsupported = resign && (!loggedIn || selectedArtifact?.metadata.extensions > 0);
  $("#confirm-install").disabled = !$("#install-device").value || unsupported;
  if (unsupported) $("#install-error").textContent = !loggedIn ? "Zaloguj się i wybierz zespół na ekranie Konto Apple." : "Podpisywanie rozszerzeń nie jest jeszcze dostępne. Użyj wcześniej podpisanej IPA.";
  else $("#install-error").textContent = "";
}
$("#install-mode").addEventListener("change",installExplanation); $("#install-device").addEventListener("change",installExplanation);
$("#close-install").addEventListener("click",() => $("#install-dialog").close());
$("#install-form").addEventListener("submit",async event => {
  event.preventDefault(); const control = $("#confirm-install"); control.disabled = true;
  const body = JSON.stringify({artifact_id:selectedArtifact.id,device_id:$("#install-device").value,mode:$("#install-mode").value});
  if (lastJobBody !== body) { requestKey = crypto.randomUUID(); lastJobBody = body; }
  try { await api("/api/jobs",{method:"POST",body,headers:{"Idempotency-Key":requestKey}}); $("#install-dialog").close(); toast("Zadanie przyjęte. Postęp znajdziesz w historii operacji."); await refresh(); $("#jobs-section").scrollIntoView({behavior:"smooth",block:"start"}); }
  catch (error) { $("#install-error").textContent = error.message + " Jeśli połączenie zostało przerwane, sprawdź historię przed ponowieniem."; }
  finally { control.disabled = false; }
});

$("#login-form").addEventListener("submit",async event => {
  event.preventDefault(); const form = event.currentTarget; const control = $("#login-button"); control.disabled = true;
  const body = {email:form.elements.email.value.trim(),password:form.elements.password.value,anisette_url:form.elements.anisette_url.value.trim(),allow_external:form.elements.allow_external.checked};
  form.elements.password.value = "";
  try { await api("/api/account/login",{method:"POST",body:JSON.stringify(body)}); await refresh(); }
  catch (error) { toast(error.message,true); }
  finally { body.password = ""; control.disabled = false; }
});
function renderAccount() {
  const view = snapshot.account; const area = $("#account-view"); area.replaceChildren();
  const titles = {logged_out:"Połącz swoje konto.",logging_in:"Logowanie do Apple…",two_factor:"Potwierdź logowanie.",logged_in:"Konto połączone.",error:"Logowanie nieudane."};
  area.append(element("span","tag",view.status === "logged_in" ? "POŁĄCZONO" : "KONTO APPLE"),element("h3","account-state",titles[view.status] || view.status));
  if (view.email) area.append(element("p","muted small",view.email));
  if (view.message) area.append(element("p","account-message error-text",view.message));
  $("#login-form").hidden = ["logging_in","two_factor","logged_in"].includes(view.status);
  if (view.status === "logging_in") area.append(element("p","muted","Czekamy na usługę anisette i odpowiedź Apple. Jeśli potrzebne jest 2FA, formularz pojawi się tutaj."));
  if (view.status === "logged_in") {
    if (!view.selected_team) {
      const form = element("form"); const label = element("label",null,"Wybierz zespół deweloperski"); const select = element("select");
      for (const team of view.teams) { const option = element("option",null,team.name); option.value = team.id; select.append(option); }
      label.append(select); const confirm = element("button","primary","Użyj tego zespołu"); confirm.type = "submit"; form.append(label,confirm);
      form.addEventListener("submit",event => { event.preventDefault(); runAction(() => api("/api/account/team",{method:"POST",body:JSON.stringify({id:select.value})}),"Wybrano zespół."); }); area.append(form);
    } else { const team = view.teams.find(t => t.id === view.selected_team); area.append(element("p","account-message","Zespół: " + (team?.name || view.selected_team))); }
    area.append(button("Wyloguj sesję Apple","secondary",() => runAction(() => api("/api/account/logout",{method:"POST"}),"Wylogowano. Certyfikaty pozostają w zaszyfrowanym magazynie do następnego odnowienia.")));
  }
  if (view.status === "two_factor" && view.challenge) {
    const current = view.challenge;
    const send = (action,value = "") => runAction(() => api("/api/account/challenge",{method:"POST",body:JSON.stringify({id:current.id,action,value})}));
    const form = element("form"); const label = element("label",null,"Kod z zaufanego urządzenia lub SMS"); const input = element("input");
    input.inputMode = "numeric"; input.autocomplete = "one-time-code"; input.pattern = "[0-9]{6}"; input.maxLength = 6; input.required = true; input.placeholder = "000000"; label.append(input);
    const confirm = element("button","primary","Potwierdź kod"); confirm.type = "submit"; form.append(label,confirm);
    form.addEventListener("submit",async event => { event.preventDefault(); const code = input.value; input.value = ""; confirm.disabled = true; try { await send("code",code); } finally { confirm.disabled = false; } }); area.append(form);
    const controls = element("div","challenge-actions"); controls.append(button("Wyślij na urządzenia","secondary",() => send("devices")),button("Wyślij ponownie","secondary",() => send("resend")),button("Anuluj logowanie","secondary",() => send("abort")));
    for (const phone of current.numbers) controls.append(button(phone.label,"secondary",() => send("sms",String(phone.id)))); area.append(controls);
  }
}

$("#export-diagnostics").addEventListener("click",async () => {
  try { const report = await api("/api/diagnostics"); const url = URL.createObjectURL(new Blob([JSON.stringify(report,null,2)],{type:"application/json"})); const anchor = element("a"); anchor.href = url; anchor.download = "MostServer-diagnostyka.json"; document.body.append(anchor); anchor.click(); anchor.remove(); setTimeout(() => URL.revokeObjectURL(url),1000); }
  catch (error) { toast(error.message,true); }
});
$("#shutdown").addEventListener("click",async () => {
  if (!window.confirm("Zakończyć działanie MostServer na tym komputerze?")) return;
  try { await api("/api/shutdown",{method:"POST"}); $("#offline").textContent = "MostServer został zamknięty. Możesz zamknąć tę kartę."; $("#offline").hidden = false; token = null; sessionStorage.removeItem("most-access-token"); }
  catch (error) { toast(error.message,true); }
});

if (token) refresh(); else showUnlock();
setInterval(refresh,2000);

$("#lan-form").addEventListener("submit",async event => {
  event.preventDefault(); const control = $("#start-lan"); control.disabled = true;
  await runAction(() => api("/api/companion/start",{method:"POST",body:JSON.stringify({address:$("#lan-address").value.trim(),port:49241})}),"MostStore LAN uruchomiony. Wygeneruj QR na ekranie Urządzenia.");
  control.disabled = snapshot?.network.enabled || false;
});
async function createPairing(device) {
  if (!snapshot?.network.enabled) { showScreen("connection"); toast("Najpierw wpisz adres LAN tego PC i uruchom MostStore LAN."); return; }
  try {
    const result = await api("/api/companion/clients",{method:"POST",body:JSON.stringify({device_id:device.id})});
    $("#pairing-qr").src = "data:image/svg+xml;base64," + btoa(result.qr_svg);
    $("#pairing-text").value = result.pairing_json;
    $("#pairing-dialog").showModal();
    await loadClients();
  } catch (error) { toast(error.message,true); }
}
$("#close-pairing").addEventListener("click",() => $("#pairing-dialog").close());
$("#pairing-dialog").addEventListener("close",() => { $("#pairing-text").value = ""; $("#pairing-qr").removeAttribute("src"); });
async function loadClients() {
  try {
    const clients = await api("/api/companion/clients"); const list = $("#client-list"); list.replaceChildren();
    if (!clients.length) { list.textContent = "Brak zaufanych połączeń MostStore."; return; }
    for (const client of clients) {
      const row = element("div","client-row"); row.append(element("span",null,client.name + " · ID …" + client.device_id.slice(-8) + " · " + date(client.created_at)));
      row.append(button("Odwołaj dostęp","secondary",async () => { await runAction(() => api("/api/companion/clients/" + encodeURIComponent(client.id),{method:"DELETE"}),"Odwołano dostęp. Kolejne żądania tego klienta zostaną odrzucone."); await loadClients(); })); list.append(row);
    }
  } catch (error) { toast(error.message,true); }
}
$("#refresh-clients").addEventListener("click",loadClients);

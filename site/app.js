"use strict";

document.documentElement.classList.add("js");

const tabs = Array.from(document.querySelectorAll('[role="tab"][data-mode]'));
const panels = Array.from(document.querySelectorAll('[role="tabpanel"]'));

function selectTab(tab, moveFocus = false) {
  for (const item of tabs) {
    const selected = item === tab;
    item.setAttribute("aria-selected", String(selected));
    item.tabIndex = selected ? 0 : -1;
  }

  for (const panel of panels) {
    panel.hidden = panel.id !== tab.getAttribute("aria-controls");
  }

  if (moveFocus) tab.focus();
}

for (const [index, tab] of tabs.entries()) {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (event) => {
    let nextIndex;
    switch (event.key) {
      case "ArrowRight":
        nextIndex = (index + 1) % tabs.length;
        break;
      case "ArrowLeft":
        nextIndex = (index - 1 + tabs.length) % tabs.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = tabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    selectTab(tabs[nextIndex], true);
  });
}

if (tabs.length > 0) selectTab(tabs[0]);

async function loadDownloads() {
  try {
    const response = await fetch("downloads.json", {cache:"no-store"});
    if (!response.ok) return;
    const manifest = await response.json();
    if (manifest.schema !== 1 || manifest.status !== "ready" || !/^\d+\.\d+\.\d+$/.test(manifest.version)) return;
    const prefix = "https://github.com/TheSuper123/qwerty123/releases/download/v" + manifest.version + "/";
    const links = [["download-zip","MostServer-windows-x64.zip"],["download-exe","MostServer.exe"],["download-ios","MostStore-unsigned.ipa"]];
    if (!links.every(([,name]) => manifest.files[name]?.url === prefix + name && /^[a-f0-9]{64}$/.test(manifest.files[name]?.sha256))) return;
    for (const [id,name] of links) {
      const link = document.getElementById(id); link.href = manifest.files[name].url; link.hidden = false;
    }
    document.getElementById("download-version").textContent = "PREVIEW " + manifest.version;
    document.getElementById("download-status").textContent = "Paczki wersji " + manifest.version + " zostały zbudowane przez GitHub Actions na Windows i macOS. ZIP: " + (manifest.files["MostServer-windows-x64.zip"].bytes / (1024 * 1024)).toFixed(1) + " MiB. Test na fizycznym iPhonie pozostaje do wykonania.";
    document.getElementById("download-sha").textContent = manifest.files["MostServer-windows-x64.zip"].sha256;
  } catch { /* Static project content remains usable when the manifest is unavailable. */ }
}
loadDownloads();

async function loadBuildStatus() {
  try {
    const response = await fetch("build-status.json", {cache:"no-store"});
    if (!response.ok) return;
    const status = await response.json();
    if (status.status === "failed" && document.getElementById("download-zip").hidden) {
      document.getElementById("download-status").textContent = "Ostatnia kompilacja lub publikacja nie zakończyła się powodzeniem. Paczki nie zostały potwierdzone. Szczegóły znajdziesz w GitHub Actions; przyciski pobierania pozostają nieaktywne.";
    }
  } catch { /* Build metadata is optional and never turns an unverified download on. */ }
}
loadBuildStatus();

"use strict";

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const $ = (id) => document.getElementById(id);
const input = $("input");
const output = $("output");
const setupMic = $("setup-mic");
let setupAvailable = false;
let captureInstalled = false;
const captureId = "das:windows-capture";
const start = $("start");
const error = $("error");
let project;
let readOnly = false;
let playing = null;
let switching = false;
let importing = false;
let statusPending = false;
let pollVersion = 0;
let pending = new Set();

function showError(message) {
  error.textContent = String(message);
  error.hidden = !message;
}

const padButtons = Array.from({ length: 12 }, (_, i) => {
  const block = document.createElement("article");
  const play = document.createElement("button");
  const assign = document.createElement("button");
  block.className = "pad";
  play.className = "trigger";
  assign.className = "assign";
  assign.textContent = "Assign audio…";
  assign.setAttribute("aria-label", "Assign audio to pad " + (i + 1));
  block.append(play, assign);
  $("pads").append(block);
  play.onclick = async () => {
    if (pending.has(i) || playing === null) return;
    pending.add(i);
    render();
    try {
      if (playing & (1 << i)) await invoke("stop_pad", { pad: i });
      else await invoke("play_pad", { pad: i });
      await pollStatus();
    } catch (e) { showError(e); }
    finally { pending.delete(i); render(); }
  };
  assign.onclick = async () => {
    if (pending.has(i)) return;
    const path = await open({ filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "aiff", "aif", "ogg"] }] }).catch(showError);
    if (typeof path !== "string") return;
    pending.add(i);
    render();
    try { project = await invoke("assign_sound", { pad: i, path }); }
    catch (e) { showError(e); }
    finally { pending.delete(i); render(); }
  };
  return { play, assign };
});

function render() {
  const running = playing !== null;
  start.textContent = running ? "Stop audio" : "Start audio";
  start.disabled = switching || (!running && !output.value);
  $("stop").disabled = !running;
  input.disabled = switching || running || output.value === captureId;
  output.disabled = switching || running;
  $("refresh").disabled = switching || running;
  setupMic.hidden = !setupAvailable || captureInstalled;
  setupMic.disabled = switching || running || !input.value;
  $("import").disabled = readOnly || importing || running || !project || project.pads.some(Boolean);
  for (let i = 0; i < padButtons.length; i++) {
    const { play, assign } = padButtons[i];
    const item = project?.pads[i];
    const active = !!(playing & (1 << i));
    const label = item?.name || "Pad " + (i + 1);
    play.textContent = pending.has(i) ? "Working…" : label;
    play.title = label;
    play.setAttribute("aria-pressed", String(active));
    play.disabled = !running || !item || pending.has(i);
    assign.disabled = readOnly || importing || pending.has(i);
  }
}

async function scan(initial = false) {
  const [inputs, outputs] = await Promise.all([
    invoke("devices", { input: true }),
    invoke("devices", { input: false })
  ]);
  for (const [select, list, saved] of [
    [input, inputs, project.input_id], [output, outputs, project.output_id]
  ]) {
    const wanted = initial ? saved : select.value;
    select.length = 1;
    for (const dev of list) select.add(new Option(dev.name, dev.id));
    if (list.some(dev => dev.id === wanted)) select.value = wanted;
    else if (select === output && !wanted) {
      const virtual = list.find(dev => dev.virtual_mic);
      if (virtual) select.value = virtual.id;
    }
  }
  captureInstalled = outputs.some(dev => dev.id === captureId);
  $("guide").hidden = !outputs.some(dev => dev.virtual_mic) || output.value === captureId;
  if (setupAvailable && !captureInstalled) {
    $("status").textContent = "Choose your hardware microphone and click Set up microphone routing.";
    render();
    return;
  }
  if ($("guide").hidden && !outputs.some(dev => dev.id === captureId)) {
    $("status").textContent = "Virtual microphone unavailable. Run the DontAbuseSounds installer to add it.";
  }
  render();
}
async function pollStatus() {
  if (statusPending || switching || playing === null) return;
  statusPending = true;
  const version = pollVersion;
  try {
    const bits = await invoke("audio_status");
    if (version === pollVersion) playing = bits;
  } catch (e) {
    if (version === pollVersion) {
      playing = null;
      $("status").textContent = "Audio stopped.";
      showError(e);
    }
  } finally { statusPending = false; render(); }
}
start.onclick = async () => {
  switching = true;
  pollVersion++;
  render();
  showError("");
  try {
    if (playing !== null) {
      await invoke("stop_audio");
      playing = null;
      $("status").textContent = "Audio stopped.";
    } else {
      await invoke("start_audio", { input: output.value === captureId ? null : input.value || null, output: output.value });
      playing = 0;
      $("status").textContent = output.value === captureId
        ? "Clips are sent to the installed capture effect. Keep the hardware microphone selected in Discord."
        : input.value ? "Microphone and pads are routed to the selected output." : "Pads are routed to the selected output.";
    }
  } catch (e) { showError(e); }
  finally { switching = false; render(); }
};
setupMic.onclick = async () => {
  if (!input.value) return;
  switching = true;
  showError("");
  $("status").textContent = "Waiting for Windows microphone setup…";
  render();
  try {
    const restart = await invoke("setup_capture", { input: input.value });
    await scan();
    if (!captureInstalled) throw new Error("Windows configured the effect, but this account cannot use its audio feed.");
    output.value = captureId;
    await invoke("start_audio", { input: null, output: captureId });
    playing = 0;
    $("status").textContent = restart
      ? "Restart Windows to load the microphone effect."
      : "Microphone routing configured. Reopen Discord’s microphone test and try a pad.";
  } catch (e) {
    $("status").textContent = "Microphone setup did not finish.";
    showError(e);
  } finally { switching = false; render(); }
};
input.onchange = render;
output.onchange = render;
$("refresh").onclick = () => scan().catch(showError);
$("stop").onclick = async () => {
  try { await invoke("stop_pad", { pad: null }); await pollStatus(); }
  catch (e) { showError(e); }
};
$("import").onclick = async () => {
  const path = await open({ filters: [{ name: "Virtual Mixer project", extensions: ["json"] }] }).catch(showError);
  if (typeof path !== "string") return;
  importing = true;
  render();
  try { project = await invoke("import_project", { path }); }
  catch (e) { showError(e); }
  finally { importing = false; render(); }
};

(async () => {
  const [state, canSetUp] = await Promise.all([invoke("settings"), invoke("capture_setup_available")]);
  setupAvailable = canSetUp;
  project = state.settings;
  readOnly = !!state.load_error;
  if (readOnly) showError("Project could not be loaded. Original preserved. " + state.load_error);
  render();
  await scan(true);
  if (setupAvailable && captureInstalled && (!output.value || output.value === captureId)) {
    output.value = captureId;
    await start.onclick();
  }
  setInterval(pollStatus, 150);
})().catch(showError);

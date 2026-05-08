let currentTweetData = null;
let selectedQuality  = "720p";

document.getElementById("qualityPills").addEventListener("click", (e) => {
  const pill = e.target.closest(".quality-pill");
  if (!pill) return;
  document.querySelectorAll(".quality-pill").forEach((p) => p.classList.remove("active"));
  pill.classList.add("active");
  selectedQuality = pill.textContent.trim();
  if (currentTweetData) loadVideoPreview();
});

document.getElementById("twitterUrl").addEventListener("keydown", (e) => {
  if (e.key === "Enter") handleFetch();
});

document.getElementById("optCaptions").addEventListener("change", () => {
  const on = document.getElementById("optCaptions").checked;
  document.getElementById("videoCaption").style.display = on ? "flex" : "none";
});

async function handleFetch() {
  const url = document.getElementById("twitterUrl").value.trim();
  if (!url) return shakeInput();
  if (!isValidTwitterUrl(url)) {
    showError("please enter a valid x.com or twitter.com link.");
    return;
  }

  showLoading("fetching tweet data...");
  hideError();

  try {
    const data = await fetchTweetInfo(url);
    currentTweetData = data;
    renderResult(data);
    document.getElementById("resultSection").scrollIntoView({ behavior: "smooth", block: "start" });
    loadVideoPreview();
  } catch (err) {
    showError(err.message || "could not fetch tweet. is the link public?");
  } finally {
    hideLoading();
  }
}

function loadVideoPreview() {
  if (!currentTweetData) return;

  const player  = document.getElementById("videoPlayer");
  const overlay = document.getElementById("videoLoadingOverlay");

  const variant = currentTweetData.variants.find(v => v.label === selectedQuality)
    || currentTweetData.variants[0];

  if (!variant) return;

  player.style.display  = "none";
  overlay.style.display = "flex";

  const proxyUrl = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;
  player.src = proxyUrl;

  player.oncanplay = () => {
    overlay.style.display = "none";
    player.style.display  = "block";
  };

  player.onerror = () => {
    overlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable — use download below</p>`;
  };

  player.load();
}

async function handleDownload() {
  if (!currentTweetData) return;

  const url    = document.getElementById("twitterUrl").value.trim();
  const author = currentTweetData.author;
  const opts   = {
    includeQuoted: document.getElementById("optQuoted").checked,
    includeReply:  document.getElementById("optReply").checked,
  };

  try {
    showLoading("downloading video...");
    const blob = await fetchVideoStream(url, selectedQuality, opts);
    triggerBlobDownload(blob, getFilenameFromTweet(author, "mp4"));
    incrementStat("totalCount");
    if (opts.includeQuoted) incrementStat("quoteCount");
    if (opts.includeReply)  incrementStat("replyCount");
  } catch (err) {
    showError(err.message || "download failed. try again.");
  } finally {
    hideLoading();
  }
}

function renderResult(data) {
  document.getElementById("panelEmpty").style.display  = "none";
  document.getElementById("panelLoaded").style.display = "flex";

  document.getElementById("videoAuthor").textContent = `@${data.author || "unknown"}`;
  document.getElementById("videoDate").textContent   = data.created_at || "—";
  document.getElementById("videoText").textContent   = data.text || "";

  document.getElementById("captionAuthor").textContent = `@${data.author || "unknown"}`;
  document.getElementById("captionText").textContent   = data.text || "";

  document.getElementById("optCaptions").checked        = false;
  document.getElementById("videoCaption").style.display = "none";

  const hasQuote = !!data.quoted_tweet;
  const hasReply = !!data.in_reply_to;
  document.getElementById("badgeQuoted").style.display = hasQuote ? "inline-flex" : "none";
  document.getElementById("badgeReply").style.display  = hasReply ? "inline-flex" : "none";
  document.getElementById("optQuoted").checked = hasQuote;
  document.getElementById("optReply").checked  = hasReply;

  if (data.variants && data.variants.length > 0) {
    const container = document.getElementById("qualityPills");
    container.innerHTML = "";
    const preferred = data.variants.find(v => v.label === "720p") || data.variants[0];
    selectedQuality = preferred.label;
    data.variants.forEach((v) => {
      const btn       = document.createElement("button");
      btn.className   = "quality-pill" + (v.label === selectedQuality ? " active" : "");
      btn.textContent = v.label;
      container.appendChild(btn);
    });
  }
}

// helpers
function incrementStat(id) {
  const el = document.getElementById(id);
  if (el) el.textContent = (parseInt(el.textContent) || 0) + 1;
}

function isValidTwitterUrl(url) {
  return /^https?:\/\/(twitter\.com|x\.com)\/.+\/status\/\d+/.test(url);
}

function shakeInput() {
  const bar = document.getElementById("inputBar");
  bar.style.animation   = "none";
  bar.offsetHeight;
  bar.style.animation   = "shake 300ms ease";
  bar.style.borderColor = "var(--error)";
  setTimeout(() => { bar.style.borderColor = ""; bar.style.animation = ""; }, 500);
}

function showLoading(msg) {
  document.getElementById("loadingText").textContent    = msg;
  document.getElementById("loadingState").style.display = "flex";
}
function hideLoading() { document.getElementById("loadingState").style.display = "none"; }

function showError(msg) {
  document.getElementById("errorText").textContent     = msg;
  document.getElementById("errorState").style.display = "flex";
}
function hideError() { document.getElementById("errorState").style.display = "none"; }

const style = document.createElement("style");
style.textContent = `
  @keyframes shake {
    0%,100% { transform: translateX(0); }
    20%      { transform: translateX(-6px); }
    40%      { transform: translateX(6px); }
    60%      { transform: translateX(-4px); }
    80%      { transform: translateX(4px); }
  }
`;
document.head.appendChild(style);
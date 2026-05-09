let currentTweetData = null;
let selectedQuality  = "720p";

// ── quality pill clicks ────────────────────────────────────────────────────
document.getElementById("qualityPills").addEventListener("click", (e) => {
  const pill = e.target.closest(".quality-pill");
  if (!pill) return;
  document.querySelectorAll(".quality-pill").forEach((p) => p.classList.remove("active"));
  pill.classList.add("active");
  selectedQuality = pill.textContent.trim();
  if (currentTweetData) loadVideoPreview();
});

// ── enter key ─────────────────────────────────────────────────────────────
document.getElementById("twitterUrl").addEventListener("keydown", (e) => {
  if (e.key === "Enter") handleFetch();
});

// ── captions toggle: swap between plain player and tweet card ─────────────
document.getElementById("optCaptions").addEventListener("change", () => {
  setCaptionMode(document.getElementById("optCaptions").checked);
});

function setCaptionMode(on) {
  const thumbArea = document.getElementById("videoThumbArea");
  const tweetCard = document.getElementById("tweetCard");
  const player    = document.getElementById("videoPlayer");
  const cardPlayer = document.getElementById("tweetCardPlayer");

  if (on) {
    thumbArea.style.display = "none";
    tweetCard.classList.add("visible");
    // sync playback position
    const t = player.currentTime;
    cardPlayer.currentTime = t;
    if (!player.paused) cardPlayer.play();
  } else {
    tweetCard.classList.remove("visible");
    thumbArea.style.display = "flex";
    // sync back
    const t = cardPlayer.currentTime;
    player.currentTime = t;
    if (!cardPlayer.paused) player.play();
  }
}

// ── fetch ──────────────────────────────────────────────────────────────────
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

// ── load video into both players via proxy ─────────────────────────────────
function loadVideoPreview() {
  if (!currentTweetData) return;

  const variant = currentTweetData.variants.find(v => v.label === selectedQuality)
    || currentTweetData.variants[0];
  if (!variant) return;

  const proxyUrl = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;

  // plain player
  const player  = document.getElementById("videoPlayer");
  const overlay = document.getElementById("videoLoadingOverlay");
  player.style.display  = "none";
  overlay.style.display = "flex";
  player.src = proxyUrl;
  player.oncanplay = () => {
    overlay.style.display = "none";
    player.style.display  = "block";
  };
  player.onerror = () => {
    overlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable — use download below</p>`;
  };
  player.load();

  // tweet card player
  const cardPlayer  = document.getElementById("tweetCardPlayer");
  const cardOverlay = document.getElementById("tweetCardLoadingOverlay");
  cardPlayer.style.display  = "none";
  cardOverlay.style.display = "flex";
  cardPlayer.src = proxyUrl;
  cardPlayer.oncanplay = () => {
    cardOverlay.style.display = "none";
    cardPlayer.style.display  = "block";
  };
  cardPlayer.onerror = () => {
    cardOverlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable</p>`;
  };
  cardPlayer.load();
}

// ── download ───────────────────────────────────────────────────────────────
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

// ── render result ──────────────────────────────────────────────────────────
function renderResult(data) {
  document.getElementById("panelEmpty").style.display  = "none";
  document.getElementById("panelLoaded").style.display = "flex";

  const cleanText = (data.text || "").replace(/\s*https:\/\/t\.co\/\S+/g, "").trim();

  // plain meta strip
  document.getElementById("videoAuthor").textContent = `@${data.author || "unknown"}`;
  document.getElementById("videoDate").textContent   = data.created_at || "—";
  document.getElementById("videoText").textContent   = cleanText;

  // tweet card fields
  const placeholder = document.getElementById("tweetCardAvatar");
  const img         = document.getElementById("tweetCardAvatarImg");
  if (data.avatar_url) {
    placeholder.style.display = "none";
    img.src = data.avatar_url;
    img.style.display = "block";
  } else {
    const initial = (data.author || "?")[0].toUpperCase();
    placeholder.textContent = initial;
    placeholder.style.display = "flex";
    img.style.display = "none";
  }
  // use author as display name since syndication API doesn't return display name separately
  document.getElementById("tweetCardName").textContent    = data.author || "unknown";
  document.getElementById("tweetCardHandle").textContent  = `@${data.author || "unknown"}`;
  document.getElementById("tweetCardText").textContent    = cleanText;
  document.getElementById("tweetCardTime").textContent    = data.created_at || "—";
  document.getElementById("tweetCardLikes").textContent   = data.likes != null ? Number(data.likes).toLocaleString() : "—";

  // reset caption mode to off
  document.getElementById("optCaptions").checked = false;
  setCaptionMode(false);

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

// ── helpers ────────────────────────────────────────────────────────────────
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

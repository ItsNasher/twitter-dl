let currentTweetData = null;
let selectedQuality  = "720p";
let cardPlayerReady  = false;
let replyPlayerReady = false;
let quotePlayerReady = false;

// quality options
document.getElementById("qualityPills").addEventListener("click", (e) => {
  const pill = e.target.closest(".quality-pill");
  if (!pill) return;
  document.querySelectorAll(".quality-pill").forEach((p) => p.classList.remove("active"));
  pill.classList.add("active");
  selectedQuality = pill.textContent.trim();
  if (currentTweetData) {
    document.getElementById("optCaptions").checked = false;
    document.getElementById("optReply").checked = false;
    setCaptionMode(false);
    loadVideoPreview();
  }
});

document.getElementById("twitterUrl").addEventListener("keydown", (e) => {
  if (e.key === "Enter") handleFetch();
});

// captions toggle
document.getElementById("optCaptions").addEventListener("change", () => {
  setCaptionMode(document.getElementById("optCaptions").checked);
});

// reply toggle
document.getElementById("optReply").addEventListener("change", () => {
  if (document.getElementById("optCaptions").checked) {
    updateReplyContext();
  }
});

// quoted toggle
document.getElementById("optQuoted").addEventListener("change", () => {
  if (document.getElementById("optCaptions").checked) {
    updateQuoteContext();
  }
});

function setCaptionMode(on) {
  const thumbArea     = document.getElementById("videoThumbArea");
  const tweetCard     = document.getElementById("tweetCard");
  const player        = document.getElementById("videoPlayer");
  const cardPlayer    = document.getElementById("tweetCardPlayer");
  const quotePlayer   = document.getElementById("quotePlayer");
  const hasOwnVideo   = currentTweetData?.variants?.length > 0;
  const hasQuoteVideo = currentTweetData?.quoted_tweet?.variants?.length > 0;

  player.pause();
  cardPlayer.pause();
  quotePlayer.pause();

  if (on) {
    thumbArea.style.display = "none";
    tweetCard.classList.add("visible");
    if (hasOwnVideo) {
      if (!cardPlayerReady) {
        loadCardPlayer();
      } else {
        cardPlayer.currentTime = player.currentTime;
      }
    }
    updateReplyContext();
    updateQuoteContext();
    if (!hasOwnVideo && hasQuoteVideo && quotePlayerReady) {
      quotePlayer.currentTime = player.currentTime;
    }
  } else {
    tweetCard.classList.remove("visible");
    thumbArea.style.display = "flex";
    if (cardPlayerReady) {
      player.currentTime = cardPlayer.currentTime;
    } else if (quotePlayerReady && !hasOwnVideo && hasQuoteVideo) {
      player.currentTime = quotePlayer.currentTime;
    }
    document.getElementById("replyContext").style.display = "none";
    document.getElementById("quoteCard").style.display    = "none";
    document.getElementById("threadLine").style.display   = "none";
    document.getElementById("tweetCard").classList.remove("has-quote-active");
  }
}

function updateReplyContext() {
  const replyContext = document.getElementById("replyContext");
  const tweetCard    = document.getElementById("tweetCard");
  const hasReply     = !!currentTweetData?.in_reply_to;
  const replyChecked = document.getElementById("optReply").checked;

  if (hasReply && replyChecked) {
    replyContext.style.display = "flex";
    tweetCard.classList.add("has-reply-active");
    document.getElementById("threadLine").style.display = "block";
    requestAnimationFrame(updateThreadLine);
    if (!replyPlayerReady) loadReplyPlayer();
  } else {
    replyContext.style.display = "none";
    tweetCard.classList.remove("has-reply-active");
    document.getElementById("threadLine").style.display = "none";
  }
}

function updateThreadLine() {
  const threadLine = document.getElementById("threadLine");
  if (threadLine.style.display === "none") return;

  const card          = document.getElementById("tweetCard");
  const replyTopRow   = document.querySelector("#replyContext .reply-top-row");
  if (!replyTopRow) return;

  const cardRect   = card.getBoundingClientRect();
  const rowRect    = replyTopRow.getBoundingClientRect();
  const endY       = rowRect.top - cardRect.top + 10;
  const startY     = 60;
  const height     = Math.max(0, endY - startY);
  threadLine.style.height = height + "px";
}

function updateQuoteContext() {
  const quoteCard    = document.getElementById("quoteCard");
  const tweetCard    = document.getElementById("tweetCard");
  const hasQuote     = !!currentTweetData?.quoted_tweet;
  const quoteChecked = document.getElementById("optQuoted").checked;

  if (hasQuote && quoteChecked) {
    quoteCard.style.display = "block";
    tweetCard.classList.add("has-quote-active");
    if (!quotePlayerReady) loadQuotePlayer();
  } else {
    quoteCard.style.display = "none";
    tweetCard.classList.remove("has-quote-active");
  }
}

function loadQuotePlayer() {
  const quote = currentTweetData?.quoted_tweet;
  if (!quote) return;

  const variant = (quote.variants || []).find(v => v.label === selectedQuality)
    || (quote.variants || [])[0];

  const wrap    = document.getElementById("quoteVideoWrap");
  const overlay = document.getElementById("quoteVideoOverlay");
  const player  = document.getElementById("quotePlayer");

  if (!variant) {
    wrap.style.display = "none";
    quotePlayerReady   = true;
    return;
  }

  wrap.style.display    = "block";
  overlay.style.display = "flex";
  player.style.display  = "block";

  const proxyUrl = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;
  player.removeAttribute("src");
  player.load();
  player.src = proxyUrl;

  player.addEventListener("canplay", () => {
    overlay.style.display = "none";
    quotePlayerReady = true;
  }, { once: true });
  player.addEventListener("error", () => {
    overlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable</p>`;
    quotePlayerReady = true;
  }, { once: true });
  player.load();
}

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

// preview
function loadVideoPreview() {
  if (!currentTweetData) return;

  const player   = document.getElementById("videoPlayer");
  const overlay  = document.getElementById("videoLoadingOverlay");

  // always clear old video state first
  player.pause();
  player.removeAttribute("src");
  player.load();
  player.style.display  = "none";
  overlay.style.display = "flex";
  overlay.innerHTML     = `<p class="loading-text">loading preview...</p>`;

  let variant = currentTweetData.variants?.find(v => v.label === selectedQuality)
    || currentTweetData.variants?.[0]
    || currentTweetData.quoted_tweet?.variants?.find(v => v.label === selectedQuality)
    || currentTweetData.quoted_tweet?.variants?.[0];
  if (!variant) {
    overlay.innerHTML = `<p class="loading-text" style="color:#71767b">no video available</p>`;
    setOptionsLocked(false);
    return;
  }

  const proxyUrl = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;

  setOptionsLocked(true);

  player.src = proxyUrl;
  player.addEventListener("canplay", () => {
    overlay.style.display = "none";
    player.style.display  = "block";
    setOptionsLocked(false);
  }, { once: true });
  player.addEventListener("error", () => {
    overlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable — use download below</p>`;
    setOptionsLocked(false);
  }, { once: true });
  player.load();

  // reset card, reply, and quote players
  const cardPlayer = document.getElementById("tweetCardPlayer");
  cardPlayer.pause();
  cardPlayer.removeAttribute("src");
  cardPlayer.load();
  cardPlayerReady = false;

  const replyPlayer = document.getElementById("replyPlayer");
  replyPlayer.pause();
  replyPlayer.removeAttribute("src");
  replyPlayer.load();
  replyPlayerReady = false;

  const quotePlayer = document.getElementById("quotePlayer");
  quotePlayer.pause();
  quotePlayer.removeAttribute("src");
  quotePlayer.load();
  quotePlayerReady = false;
}

function setOptionsLocked(locked) {
  const items = [
    { rowId: "optCaptionsRow", checkId: "optCaptions", available: true },
    { rowId: "optQuotedRow",   checkId: "optQuoted",   available: !!currentTweetData?.quoted_tweet },
    { rowId: "optReplyRow",    checkId: "optReply",    available: !!currentTweetData?.in_reply_to },
  ];
  items.forEach(({ rowId, checkId, available }) => {
    const row   = document.getElementById(rowId);
    const check = document.getElementById(checkId);
    if (!row || !check) return;
    if (locked) {
      row.classList.add("option-row-disabled");
      check.disabled = true;
    } else if (available) {
      row.classList.remove("option-row-disabled");
      check.disabled = false;
    }
  });
}

function loadCardPlayer() {
  const variant = currentTweetData.variants.find(v => v.label === selectedQuality)
    || currentTweetData.variants[0];
  if (!variant) {
    cardPlayerReady = true;
    return;
  }

  const proxyUrl    = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;
  const cardPlayer  = document.getElementById("tweetCardPlayer");
  const cardOverlay = document.getElementById("tweetCardLoadingOverlay");
  const plainPlayer = document.getElementById("videoPlayer");

  cardOverlay.style.display    = "flex";
  cardPlayer.style.display     = "block";
  cardPlayer.removeAttribute("src");
  cardPlayer.load();

  cardPlayer.src = proxyUrl;
  cardPlayer.addEventListener("canplay", () => {
    cardOverlay.style.display = "none";
    cardPlayerReady = true;
    cardPlayer.currentTime = plainPlayer.currentTime;
    requestAnimationFrame(updateThreadLine);
  }, { once: true });
  cardPlayer.addEventListener("error", () => {
    cardOverlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable</p>`;
  }, { once: true });
  cardPlayer.load();
}

// reply player
function loadReplyPlayer() {
  const reply = currentTweetData?.in_reply_to;
  if (!reply) return;

  const variant = reply.variants.find(v => v.label === selectedQuality)
    || reply.variants[0];

  const wrap    = document.getElementById("replyVideoWrap");
  const overlay = document.getElementById("replyVideoOverlay");
  const player  = document.getElementById("replyPlayer");

  if (!variant) {
    wrap.style.display = "none";
    replyPlayerReady   = true;
    return;
  }

  wrap.style.display    = "block";
  overlay.style.display = "flex";
  player.style.display  = "block";

  const proxyUrl = `http://localhost:3000/api/preview?url=${encodeURIComponent(variant.url)}`;
  player.removeAttribute("src");
  player.load();
  player.src = proxyUrl;

  player.addEventListener("canplay", () => {
    overlay.style.display = "none";
    replyPlayerReady = true;
    requestAnimationFrame(updateThreadLine);
  }, { once: true });
  player.addEventListener("error", () => {
    overlay.innerHTML = `<p class="loading-text" style="color:var(--error)">preview unavailable</p>`;
  }, { once: true });
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

  const cleanText = (data.text || "").replace(/\s*https:\/\/t\.co\/\S+/g, "").trim();

  document.getElementById("videoAuthor").textContent = `@${data.author || "unknown"}`;
  document.getElementById("videoDate").textContent   = data.created_at || "—";
  document.getElementById("videoText").textContent   = cleanText;

  // main tweet card
  const placeholder = document.getElementById("tweetCardAvatar");
  const img         = document.getElementById("tweetCardAvatarImg");
  if (data.avatar_url) {
    placeholder.style.display = "none";
    img.src = data.avatar_url;
    img.style.display = "block";
  } else {
    placeholder.textContent   = ((data.display_name || data.author) || "?")[0].toUpperCase();
    placeholder.style.display = "flex";
    img.style.display         = "none";
  }
  document.getElementById("tweetCardName").textContent   = data.display_name || data.author || "unknown";
  document.getElementById("tweetCardHandle").textContent = `@${data.author || "unknown"}`;
  document.getElementById("tweetCardText").textContent   = cleanText;
  document.getElementById("tweetCardTime").textContent   = data.created_at || "—";
  document.getElementById("tweetCardLikes").textContent  = data.likes != null ? Number(data.likes).toLocaleString() : "—";

  // reply context mini-card content
  if (data.in_reply_to) {
    const r = data.in_reply_to;
    let rClean = (r.text || "").replace(/\s*https:\/\/t\.co\/\S+/g, "").trim();
    rClean = rClean.replace(/^@\S+\s+/, "").trim();
    const rImg   = document.getElementById("replyAvatarImg");
    const rPh    = document.getElementById("replyAvatarPlaceholder");
    if (r.avatar_url) {
      rImg.src = r.avatar_url;
      rImg.style.display = "block";
      rPh.style.display  = "none";
    } else {
      rPh.textContent   = ((r.display_name || r.author) || "?")[0].toUpperCase();
      rPh.style.display = "flex";
      rImg.style.display = "none";
    }
    document.getElementById("replyDisplayName").textContent = r.display_name || r.author || "unknown";
    document.getElementById("replyHandle").textContent      = `@${r.author || "unknown"}`;
    document.getElementById("replyText").textContent        = rClean;
    document.getElementById("replyTimestamp").textContent   = r.created_at || "";
  }

  // quoted tweet card content
  if (data.quoted_tweet) {
    const q = data.quoted_tweet;
    const qClean = (q.text || "").replace(/\s*https:\/\/t\.co\/\S+/g, "").trim();
    const qImg = document.getElementById("quoteAvatarImg");
    const qPh  = document.getElementById("quoteAvatarPlaceholder");
    if (q.avatar_url) {
      qImg.src = q.avatar_url;
      qImg.style.display = "block";
      qPh.style.display  = "none";
    } else {
      qPh.textContent   = ((q.display_name || q.author) || "?")[0].toUpperCase();
      qPh.style.display = "flex";
      qImg.style.display = "none";
    }
    document.getElementById("quoteDisplayName").textContent = q.display_name || q.author || "unknown";
    document.getElementById("quoteHandle").textContent      = `@${q.author || "unknown"}`;
    document.getElementById("quoteDate").textContent        = q.created_at || "";
    document.getElementById("quoteText").textContent        = qClean;
  }

  // reset states — fully tear down all players before new data
  document.getElementById("optCaptions").checked        = false;
  document.getElementById("replyContext").style.display = "none";
  document.getElementById("quoteCard").style.display    = "none";
  document.getElementById("threadLine").style.display   = "none";
  document.getElementById("tweetCard").classList.remove("has-quote-active");
  replyPlayerReady = false;
  quotePlayerReady = false;
  setCaptionMode(false);

  setOptionEnabled("optQuotedRow", "optQuoted", "badgeQuoted", !!data.quoted_tweet);
  setOptionEnabled("optReplyRow",  "optReply",  "badgeReply",  !!data.in_reply_to);

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

// option row helper
function setOptionEnabled(rowId, checkId, badgeId, available) {
  const row   = document.getElementById(rowId);
  const check = document.getElementById(checkId);
  const badge = document.getElementById(badgeId);

  if (available) {
    row.classList.remove("option-row-disabled");
    check.disabled = false;
    check.checked  = true;
    badge.style.display = "inline-flex";
  } else {
    row.classList.add("option-row-disabled");
    check.disabled = true;
    check.checked  = false;
    badge.style.display = "none";
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
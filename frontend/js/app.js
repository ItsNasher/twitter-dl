let currentTweetData = null;
let selectedQuality = "1080p";

document.getElementById("qualityPills").addEventListener("click", (e) => {
  const pill = e.target.closest(".quality-pill");
  if (!pill) return;
  document
    .querySelectorAll(".quality-pill")
    .forEach((p) => p.classList.remove("active"));
  pill.classList.add("active");
  selectedQuality = pill.textContent.trim();
});

document.getElementById("twitterUrl").addEventListener("keydown", (e) => {
  if (e.key === "Enter") handleFetch();
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
    document
      .getElementById("resultSection")
      .scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (err) {
    showError(err.message || "could not fetch tweet. is the link public?");
  } finally {
    hideLoading();
  }
}

async function handleDownload(type) {
  if (!currentTweetData) return;

  const url = document.getElementById("twitterUrl").value.trim();
  const author = currentTweetData.author;
  const includeQuoted = document.getElementById("optQuoted").checked;
  const includeReply = document.getElementById("optReply").checked;

  try {
    if (type === "video") {
      showLoading("downloading video...");
      const blob = await fetchVideoStream(url, selectedQuality, {
        includeQuoted,
        includeReply,
      });
      triggerBlobDownload(blob, getFilenameFromTweet(author, "mp4"));
      incrementStat("totalCount");
      if (includeQuoted) incrementStat("quoteCount");
      if (includeReply) incrementStat("replyCount");
    } else if (type === "captions") {
      showLoading("fetching captions...");
      const blob = await fetchCaptions(url);
      triggerBlobDownload(blob, getFilenameFromTweet(author, "srt"));
    } else if (type === "audio") {
      showLoading("extracting audio...");
      const blob = await fetchAudioOnly(url, selectedQuality);
      triggerBlobDownload(blob, getFilenameFromTweet(author, "m4a"));
    }
  } catch (err) {
    showError(err.message || "download failed. try again.");
  } finally {
    hideLoading();
  }
}

function renderResult(data) {
  document.getElementById("panelEmpty").style.display = "none";
  document.getElementById("panelLoaded").style.display = "flex";

  document.getElementById("videoAuthor").textContent =
    `@${data.author || "unknown"}`;
  document.getElementById("videoDate").textContent = data.created_at || "—";
  document.getElementById("videoText").textContent = data.text || "";

  const hasQuote = !!data.quoted_tweet;
  const hasReply = !!data.in_reply_to;

  document.getElementById("badgeQuoted").style.display = hasQuote
    ? "inline-flex"
    : "none";
  document.getElementById("badgeReply").style.display = hasReply
    ? "inline-flex"
    : "none";

  document.getElementById("optQuoted").checked = hasQuote;
  document.getElementById("optReply").checked = hasReply;

  if (data.variants && data.variants.length > 0) {
    const container = document.getElementById("qualityPills");
    container.innerHTML = "";
    data.variants.forEach((v, i) => {
      const btn = document.createElement("button");
      btn.className = "quality-pill" + (i === 0 ? " active" : "");
      btn.textContent = v.label || v.quality || "unknown";
      container.appendChild(btn);
    });
    selectedQuality = data.variants[0]?.label || "1080p";
  }
}

function incrementStat(id) {
  const el = document.getElementById(id);
  if (el) el.textContent = (parseInt(el.textContent) || 0) + 1;
}

function isValidTwitterUrl(url) {
  return /^https?:\/\/(twitter\.com|x\.com)\/.+\/status\/\d+/.test(url);
}

function shakeInput() {
  const bar = document.getElementById("inputBar");
  bar.style.animation = "none";
  bar.offsetHeight;
  bar.style.animation = "shake 300ms ease";
  bar.style.borderColor = "var(--error)";
  setTimeout(() => {
    bar.style.borderColor = "";
    bar.style.animation = "";
  }, 500);
}

function showLoading(msg) {
  document.getElementById("loadingText").textContent = msg;
  document.getElementById("loadingState").style.display = "flex";
}

function hideLoading() {
  document.getElementById("loadingState").style.display = "none";
}

function showError(msg) {
  document.getElementById("errorText").textContent = msg;
  document.getElementById("errorState").style.display = "flex";
}

function hideError() {
  document.getElementById("errorState").style.display = "none";
}

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

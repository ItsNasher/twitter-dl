  let currentTweetData = null;
  let selectedQuality = '1080p';
  
  // ---- QUALITY PILL SELECTION ----
  document.querySelectorAll('.quality-pill').forEach(pill => {
    pill.addEventListener('click', () => {
      document.querySelectorAll('.quality-pill').forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      selectedQuality = pill.textContent.trim();
    });
  });
  
  // ---- ENTER KEY ON INPUT ----
  document.getElementById('twitterUrl').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleFetch();
  });
  
  // ---- FETCH TWEET INFO ----
  async function handleFetch() {
    const url = document.getElementById('twitterUrl').value.trim();
    if (!url) return shakeInput();
  
    if (!isValidTwitterUrl(url)) {
      showError('please enter a valid x.com or twitter.com link.');
      return;
    }
  
    showLoading('fetching tweet data...');
    hideResult();
    hideError();
  
    try {
      const data = await fetchTweetInfo(url);
      currentTweetData = data;
      renderResult(data);
      renderExtras(data);
      showResult();
      scrollToResult();
    } catch (err) {
      showError(err.message || 'could not fetch tweet. is the link public?');
    } finally {
      hideLoading();
    }
  }
  
  // ---- DOWNLOAD ----
  async function handleDownload(type) {
    if (!currentTweetData) return;
  
    const url = document.getElementById('twitterUrl').value.trim();
    const author = currentTweetData.author;
    const includeQuote = document.getElementById('includeQuote').checked;
    const includeReply = document.getElementById('includeReply').checked;
  
    try {
      if (type === 'video') {
        showLoading('downloading video...');
        const blob = await fetchVideoStream(url, selectedQuality, includeQuote, includeReply);
        triggerBlobDownload(blob, getFilenameFromTweet(author, 'mp4'));
      } else if (type === 'captions') {
        showLoading('fetching captions...');
        const blob = await fetchCaptions(url, includeQuote, includeReply);
        triggerBlobDownload(blob, getFilenameFromTweet(author, 'srt'));
      } else if (type === 'audio') {
        showLoading('extracting audio...');
        const blob = await fetchAudioOnly(url, selectedQuality, includeQuote, includeReply);
        triggerBlobDownload(blob, getFilenameFromTweet(author, 'm4a'));
      }
    } catch (err) {
      showError(err.message || 'download failed. try again.');
    } finally {
      hideLoading();
    }
  }
  
  // ---- RENDER RESULT CARD ----
  function renderResult(data) {
    document.getElementById('resultAuthor').textContent = `@${data.author || 'unknown'}`;
    document.getElementById('resultDate').textContent = data.created_at || '—';
    document.getElementById('resultText').textContent = data.text || '';
  
    // Render quality pills from API variants
    if (data.variants && data.variants.length > 0) {
      const container = document.getElementById('qualityPills');
      container.innerHTML = '';
      data.variants.forEach((v, i) => {
        const btn = document.createElement('button');
        btn.className = 'quality-pill' + (i === 0 ? ' active' : '');
        btn.textContent = v.label || v.quality || 'unknown';
        btn.addEventListener('click', () => {
          document.querySelectorAll('.quality-pill').forEach(p => p.classList.remove('active'));
          btn.classList.add('active');
          selectedQuality = btn.textContent.trim();
        });
        container.appendChild(btn);
      });
      selectedQuality = data.variants[0]?.label || '1080p';
    }
  }

  // ---- RENDER EXTRAS (quote/reply checkboxes) ----
  function renderExtras(data) {
    const quoteWrap = document.getElementById('quoteCheckWrap');
    const replyWrap = document.getElementById('replyCheckWrap');
    quoteWrap.style.display = data.is_quote ? 'inline-flex' : 'none';
    replyWrap.style.display = data.is_reply ? 'inline-flex' : 'none';
    document.getElementById('includeQuote').checked = false;
    document.getElementById('includeReply').checked = false;
  }
  
  // ---- UI HELPERS ----
  function isValidTwitterUrl(url) {
    return /^https?:\/\/(twitter\.com|x\.com)\/.+\/status\/\d+/.test(url);
  }
  
  function shakeInput() {
    const bar = document.getElementById('inputBar');
    bar.style.animation = 'none';
    bar.offsetHeight;
    bar.style.animation = 'shake 300ms ease';
    bar.style.borderColor = 'var(--error)';
    setTimeout(() => { bar.style.borderColor = ''; bar.style.animation = ''; }, 500);
  }
  
  function showLoading(msg) {
    document.getElementById('loadingText').textContent = msg;
    document.getElementById('loadingState').style.display = 'flex';
  }
  
  function hideLoading() {
    document.getElementById('loadingState').style.display = 'none';
  }
  
  function showResult() {
    document.getElementById('resultSection').style.display = 'block';
    document.getElementById('resultCard').style.display = 'block';
  }
  
  function hideResult() {
    document.getElementById('resultSection').style.display = 'none';
    document.getElementById('resultCard').style.display = 'none';
  }
  
  function scrollToResult() {
    const section = document.getElementById('resultSection');
    const main = document.querySelector('.main-content');
    if (main) {
      main.scrollTo({ top: main.scrollHeight, behavior: 'smooth' });
    } else {
      section.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }
  
  function showError(msg) {
    document.getElementById('errorText').textContent = msg;
    document.getElementById('errorState').style.display = 'flex';
  }
  
  function hideError() {
    document.getElementById('errorState').style.display = 'none';
  }
  
  // ---- INJECT SHAKE KEYFRAME ----
  const style = document.createElement('style');
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
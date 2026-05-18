const API_BASE = 'http://localhost:3000/api';

async function fetchTweetInfo(url) {
  const res = await fetch(`${API_BASE}/info`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

async function fetchVideoStream(url, quality, { includeQuote, includeReply, renderCard } = {}, onProgress) {
  const res = await fetch(`${API_BASE}/download`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      url,
      quality,
      include_quote: includeQuote ?? false,
      include_reply: includeReply ?? false,
      render_card: renderCard ?? false,
    }),
  });
  if (!res.ok) throw new Error(await res.text());

  const total = parseInt(res.headers.get('Content-Length') || '0', 10);
  const reader = res.body.getReader();
  const chunks = [];
  let received = 0;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    if (onProgress) {
      onProgress(total ? received / total : -1);
    }
  }

  return new Blob(chunks, { type: 'video/mp4' });
}

async function fetchCaptions(url, { includeQuote, includeReply } = {}) {
  const res = await fetch(`${API_BASE}/captions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      url,
      include_quote: includeQuote ?? false,
      include_reply: includeReply ?? false,
    }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.blob();
}

async function fetchAudioOnly(url, quality, { includeQuote, includeReply } = {}) {
  const res = await fetch(`${API_BASE}/audio`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      url,
      quality,
      include_quote: includeQuote ?? false,
      include_reply: includeReply ?? false,
    }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.blob();
}
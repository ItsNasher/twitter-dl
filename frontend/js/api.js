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

async function fetchVideoStream(url, quality, includeQuote, includeReply) {
  const res = await fetch(`${API_BASE}/download`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, quality, include_quote: includeQuote, include_reply: includeReply }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.blob();
}

async function fetchCaptions(url, includeQuote, includeReply) {
  const res = await fetch(`${API_BASE}/captions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, include_quote: includeQuote, include_reply: includeReply }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.blob();
}

async function fetchAudioOnly(url, quality, includeQuote, includeReply) {
  const res = await fetch(`${API_BASE}/audio`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, quality, include_quote: includeQuote, include_reply: includeReply }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.blob();
}
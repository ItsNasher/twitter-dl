function triggerBlobDownload(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}

function getFilenameFromTweet(author, type = "mp4") {
  const safe = (author || "tweet").replace(/[^a-z0-9_]/gi, "_").toLowerCase();
  const ts = Date.now();
  return `${safe}_${ts}.${type}`;
}

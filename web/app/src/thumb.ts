export function thumbnail(file: File, at: number): Promise<string | null> {
  return new Promise((resolve) => {
    const video = document.createElement('video');
    const url = URL.createObjectURL(file);
    const done = (result: string | null) => {
      URL.revokeObjectURL(url);
      video.removeAttribute('src');
      video.load();
      resolve(result);
    };
    video.muted = true;
    video.preload = 'auto';
    video.onloadeddata = () => {
      video.currentTime = at;
    };
    video.onseeked = () => {
      const canvas = document.createElement('canvas');
      canvas.width = 64;
      canvas.height = 40;
      const ctx = canvas.getContext('2d');
      if (!ctx || !video.videoWidth || !video.videoHeight) return done(null);
      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, 64, 40);
      const scale = Math.min(64 / video.videoWidth, 40 / video.videoHeight);
      const w = Math.round(video.videoWidth * scale);
      const h = Math.round(video.videoHeight * scale);
      ctx.drawImage(video, Math.floor((64 - w) / 2), Math.floor((40 - h) / 2), w, h);
      done(canvas.toDataURL());
    };
    video.onerror = () => done(null);
    video.src = url;
  });
}

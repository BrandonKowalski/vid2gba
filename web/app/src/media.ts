import { ALL_FORMATS, BlobSource, Input } from 'mediabunny';

export interface MediaInfo {
  duration: number;
  width: number;
  height: number;
  videoCodec: string;
  audio: boolean;
}

const UNREADABLE = "Couldn't read this file as a video.";

export async function inspect(file: File): Promise<MediaInfo | { error: string }> {
  const input = new Input({ source: new BlobSource(file), formats: ALL_FORMATS });
  try {
    const video = await input.getPrimaryVideoTrack();
    if (!video) return { error: UNREADABLE };
    const codec = (await video.getCodec()) ?? 'this';
    if (!(await video.canDecode())) {
      return { error: `This browser can't decode ${codec} video. Re-save the file as MP4 (H.264) and try again.` };
    }
    const audioTrack = await input.getPrimaryAudioTrack();
    const audio = audioTrack ? await audioTrack.canDecode() : false;
    const duration = await input.computeDuration();
    const width = await video.getDisplayWidth();
    const height = await video.getDisplayHeight();
    return { duration, width, height, videoCodec: codec, audio };
  } catch {
    return { error: UNREADABLE };
  } finally {
    input.dispose();
  }
}

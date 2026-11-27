/**
 * Helpers for the image translation page (translate an image file the
 * user opened, pasted or dropped). Free of DOM / Tauri for unit tests.
 */

/** Extensions the backend decodes (see `capture::image_file_png`). */
export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp'];
export const IMAGE_ACCEPT = '.png,.jpg,.jpeg,.webp,image/png,image/jpeg,image/webp';
/** Same limit as `capture::MAX_IMAGE_FILE_BYTES`. */
export const MAX_IMAGE_BYTES = 40 * 1024 * 1024;

/** True for a file name / path with a supported image extension. */
export function isImageName(name: string): boolean {
  const m = /\.([a-z0-9]+)$/i.exec(name.trim());
  return !!m && IMAGE_EXTENSIONS.includes(m[1].toLowerCase());
}

/** First supported image among dropped paths, or null. */
export function firstImagePath(paths: string[]): string | null {
  return paths.find(isImageName) ?? null;
}

/** First pasted / picked file that is a supported image, or null. */
export function firstImageFile(files: ArrayLike<File> | null | undefined): File | null {
  if (!files) return null;
  for (let i = 0; i < files.length; i++) {
    const f = files[i];
    if (/^image\/(png|jpeg|webp)$/.test(f.type) || isImageName(f.name)) return f;
  }
  return null;
}

/** Base name of a path, for the page header ("menu.jpg"). */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/**
 * 枠（room）へ収まる最大の等比寸法。拡大は原寸の limit 倍まで（既定は原寸）。
 * 原寸・枠の寸法が未確定（0）の間は 0。
 */
export function containSize(
  natural: { w: number; h: number },
  room: { w: number; h: number },
  limit = 1
): { w: number; h: number } {
  if (natural.w <= 0 || natural.h <= 0 || room.w <= 0 || room.h <= 0) return { w: 0, h: 0 };
  const k = Math.min(room.w / natural.w, room.h / natural.h, limit);
  return { w: Math.floor(natural.w * k), h: Math.floor(natural.h * k) };
}

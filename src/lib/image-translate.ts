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

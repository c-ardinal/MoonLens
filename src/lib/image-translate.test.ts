import { describe, expect, it } from 'vitest';
import { baseName, firstImageFile, firstImagePath, isImageName } from './image-translate';

describe('image translate helpers', () => {
  it('accepts only the formats the backend decodes', () => {
    expect(isImageName('shot.PNG')).toBe(true);
    expect(isImageName('C:\\photos\\menu.jpeg')).toBe(true);
    expect(isImageName('a.webp')).toBe(true);
    expect(isImageName('anim.gif')).toBe(false);
    expect(isImageName('notes.txt')).toBe(false);
    expect(isImageName('png')).toBe(false);
  });

  it('picks the first image among dropped paths', () => {
    expect(firstImagePath(['/a/readme.md', '/a/b.jpg', '/a/c.png'])).toBe('/a/b.jpg');
    expect(firstImagePath(['/a/readme.md'])).toBeNull();
  });

  it('picks the first image file by type or name', () => {
    const txt = new File(['x'], 'a.txt', { type: 'text/plain' });
    const pasted = new File(['x'], 'image.png', { type: 'image/png' });
    const noType = new File(['x'], 'photo.JPG', { type: '' });
    expect(firstImageFile([txt, pasted])).toBe(pasted);
    expect(firstImageFile([noType])).toBe(noType);
    expect(firstImageFile([txt])).toBeNull();
    expect(firstImageFile(null)).toBeNull();
  });

  it('shows the base name of a path', () => {
    expect(baseName('C:\\Users\\u\\Pictures\\menu.jpg')).toBe('menu.jpg');
    expect(baseName('/home/u/shot.png')).toBe('shot.png');
  });
});

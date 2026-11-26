import { describe, expect, it } from 'vitest';
import { baseName, containSize, firstImageFile, firstImagePath, isImageName } from './image-translate';

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

describe('containSize', () => {
  it('枠へ収まる最大の等比寸法', () => {
    expect(containSize({ w: 900, h: 700 }, { w: 450, h: 700 })).toEqual({ w: 450, h: 350 });
    expect(containSize({ w: 900, h: 700 }, { w: 2000, h: 350 })).toEqual({ w: 450, h: 350 });
  });
  it('拡大は上限倍率まで', () => {
    expect(containSize({ w: 100, h: 50 }, { w: 1000, h: 1000 })).toEqual({ w: 100, h: 50 });
    expect(containSize({ w: 100, h: 50 }, { w: 1000, h: 1000 }, 2)).toEqual({ w: 200, h: 100 });
  });
  it('寸法未確定は 0', () => {
    expect(containSize({ w: 0, h: 0 }, { w: 100, h: 100 })).toEqual({ w: 0, h: 0 });
  });
});

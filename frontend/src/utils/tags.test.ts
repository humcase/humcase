import { describe, it, expect } from 'vitest';
import { readTags } from './tags';

describe('readTags — defensive tag decoding', () => {
  it('decodes an array of strings', () => {
    expect(readTags('["a","b"]')).toEqual(['a', 'b']);
  });

  it("keeps only the strings of a mixed array (white screen guard)", () => {
    expect(readTags('["ok",42,{"x":1},null]')).toEqual(['ok']);
  });

  it('returns an empty array for null, invalid JSON or non-array', () => {
    expect(readTags(null)).toEqual([]);
    expect(readTags(undefined)).toEqual([]);
    expect(readTags('')).toEqual([]);
    expect(readTags('pas du json')).toEqual([]);
    expect(readTags('{"a":1}')).toEqual([]);
    expect(readTags('"texte"')).toEqual([]);
  });
});

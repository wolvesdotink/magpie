import assert from 'node:assert/strict';
import { test } from 'node:test';
import { captionUpdate } from '../src/lib/captions.ts';

const words = (text) => text.split(' ');

test('caption revision retains the unchanged prefix', () => {
  assert.deepEqual(
    captionUpdate(words('I like cats'), words('I like cats'), words('I like dogs')),
    {
      words: words('I like dogs'),
      keep: 2,
    },
  );
});

test('shorter hypothesis keeps the surviving words', () => {
  assert.deepEqual(captionUpdate(words('I like cats'), words('I like cats'), words('I like')), {
    words: words('I like'),
    keep: 2,
  });
});

test('revision preserves a visible suffix after overflow', () => {
  assert.deepEqual(
    captionUpdate(words('one two three four'), words('three four'), words('one two three five')),
    { words: words('three five'), keep: 1 },
  );
});

test('sliding window replaces the old phrase coherently', () => {
  assert.deepEqual(
    captionUpdate(words('one two three four'), words('three four'), words('a new sentence')),
    { words: words('a new sentence'), keep: 0 },
  );
});

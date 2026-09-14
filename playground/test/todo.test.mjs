import test from 'node:test';
import assert from 'node:assert/strict';
import { createTodoList } from '../src/todo.mjs';
test('todo list adds and lists in order', () => {
  const todos = createTodoList();
  assert.equal(todos.add('write docs'), 0);
  assert.equal(todos.add('run auto'), 1);
  assert.deepEqual(todos.list(), ['write docs', 'run auto']);
});

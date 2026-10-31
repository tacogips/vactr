import { EditorState, Text } from '@codemirror/state';
import { EditorCore } from 'emaki/engine';
import { expect, it } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

it('uses one CodeMirror instance across vactr and emaki', () => {
  const client = new Client(new RecordingTransport(), { store: new Store(), now: () => Date.now() });
  const surface = new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of(['hello'])) });
  expect(surface.state).toBeInstanceOf(EditorState);
  expect(surface.state.doc).toBeInstanceOf(Text);
  expect(surface).toBeInstanceOf(EditorCore);
  surface.dispose();
});

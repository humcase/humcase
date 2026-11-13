import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import SettingsModal, { AGENT_PERMISSION_TEXT } from './SettingsModal';
import type { AiProvidersInfo } from '../types';

vi.mock('../services/tauriApi', () => ({
  api: {
    listAiProviders: vi.fn(),
    setAiKey: vi.fn(),
    setAiUrl: vi.fn(),
    testAiConnection: vi.fn(),
    getMcpPermissions: vi.fn(),
    setMcpAllowNoteEdit: vi.fn(),
    openDataDir: vi.fn(),
    backupDatabase: vi.fn(),
    createBackupArchive: vi.fn(),
    inspectBackupArchive: vi.fn(),
    restoreBackupArchive: vi.fn(),
    readAiUsage: vi.fn(async () => []),
    recheckNatureReading: vi.fn(async () => undefined),
    meaningStatus: vi.fn(() => new Promise(() => {})),
    tidySettings: vi.fn(() => new Promise(() => {})),
    natureReadingStatus: vi.fn(() => new Promise(() => {})),
    profileSettings: vi.fn(() => new Promise(() => {})),
  },
}));

vi.mock('../utils/quickCaptureBarrier', () => ({
  prepareQuickCaptureForRestore: vi.fn(),
}));

import { api } from '../services/tauriApi';
import { prepareQuickCaptureForRestore } from '../utils/quickCaptureBarrier';

const mockApi = vi.mocked(api);
const mockPrepareQuickCapture = vi.mocked(prepareQuickCaptureForRestore);

const archivePreview = {
  archive_sha256: 'sha256-archive-abc123',
  format_version: 1,
  created_at: '2026-09-23T08:45:00Z',
  database_schema_version: 9,
  project_count: 3,
  note_count: 42,
  image_count: 7,
  total_uncompressed_bytes: 123456,
};

const archivePath = 'C:\\Users\\Alex\\Documents\\Humcase Backups\\notes.humcase.zip';

async function openRestoreConfirmation(path = archivePath) {
  fireEvent.click(screen.getByRole('button', { name: 'Données' }));
  fireEvent.change(screen.getByLabelText('Restaurer depuis une archive'), { target: { value: path } });
  fireEvent.click(screen.getByRole('button', { name: 'Examiner l’archive' }));
  await screen.findByText('Archive valide');
  fireEvent.click(screen.getByRole('button', { name: 'Restaurer cette archive' }));
}

const list: AiProvidersInfo = {
  cleanup: [
    {
      id: 'cerebras',
      label: 'Cerebras',
      key_env: 'CEREBRAS_API_KEY',
      default_model: 'gpt-oss-120b',
      url_env: '',
      model_env: 'CEREBRAS_MODEL',
      needs_key: true,
      key_defined: false,
      key_source: 'none',
      url_editable: false,
      url: 'https://api.cerebras.ai/v1/chat/completions',
      ready: false,
    },
    {
      id: 'ollama',
      label: 'Ollama (local)',
      key_env: '',
      default_model: 'llama3.2',
      url_env: '',
      model_env: 'OLLAMA_MODEL',
      needs_key: false,
      key_defined: false,
      key_source: 'none',
      url_editable: true,
      url: 'http://127.0.0.1:11434/v1/chat/completions',
      ready: true,
    },
  ],
  transcription: [
    {
      id: 'deepgram',
      label: 'Deepgram',
      key_env: 'DEEPGRAM_API_KEY',
      default_model: '',
      url_env: '',
      model_env: '',
      needs_key: true,
      key_defined: true,
      key_source: 'env',
      url_editable: false,
      url: '',
      ready: true,
    },
  ],
  config_alert: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mockApi.listAiProviders.mockResolvedValue(list);
  mockApi.setAiKey.mockResolvedValue(undefined);
  mockApi.getMcpPermissions.mockResolvedValue({ allow_note_edit: false });
  mockApi.setMcpAllowNoteEdit.mockResolvedValue(undefined);
  mockApi.createBackupArchive.mockResolvedValue('C:\\Users\\Alex\\Documents\\Humcase Backups\\humcase-2026-09-23-1045.humcase.zip');
  mockApi.inspectBackupArchive.mockResolvedValue(archivePreview);
  mockApi.restoreBackupArchive.mockResolvedValue(undefined);
  mockPrepareQuickCapture.mockReturnValue({ ready: true });
});

afterEach(() => {
  localStorage.clear();
});

describe('SettingsModal — navigation', () => {
  it('shows one section at a time and allows changing the theme', () => {
    const onThemeChange = vi.fn();
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} theme="dark" onThemeChange={onThemeChange} />);

    // Six sections, Settings open on General (no overview page any more).
    const nav = screen.getByRole('navigation');
    // Six sections, then the way back to the app at the foot of the navigation.
    expect([...nav.querySelectorAll('button')].map(b => b.textContent)).toEqual(['Général', 'Dictée', 'Services IA', 'Propositions', 'Agents', 'Données', 'Revenir à l’application']);
    const general = screen.getByRole('button', { name: 'Général' });
    expect(general.getAttribute('aria-current')).toBe('page');
    expect(screen.queryByLabelText('Restaurer depuis une archive')).toBeNull();
    general.focus();
    fireEvent.click(general);
    expect(document.activeElement).toBe(general);
    expect(screen.getByLabelText("Taille du texte de l’éditeur")).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'Sombre' })).toHaveProperty('checked', true);
    fireEvent.click(screen.getByRole('radio', { name: 'Clair' }));
    expect(onThemeChange).toHaveBeenCalledWith('light');
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));
    expect(screen.getByLabelText('Restaurer depuis une archive')).toBeTruthy();
    expect(screen.queryByLabelText("Taille du texte de l’éditeur")).toBeNull();
  });
});

describe('SettingsModal — one place for every key', () => {
  it('Dictation sends to AI services, which hold the TypeSafe key too, never shown back', async () => {
    mockApi.listAiProviders.mockResolvedValue({ ...list, reading: [{ id: 'typesafe', label: 'TypeSafe', key_env: 'TYPESAFE_API_KEY', default_model: '', url_env: '', model_env: '', needs_key: true, key_defined: false, key_source: 'none', url_editable: false, url: '', ready: false }] });
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Dictée' }));
    expect(await screen.findByText('Deepgram · branché')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir Services IA' }));
    expect(screen.getByRole('button', { name: 'Services IA' }).getAttribute('aria-current')).toBe('page');
    const key = screen.getByLabelText('Relecture des propositions (TypeSafe)') as HTMLInputElement;
    fireEvent.change(key, { target: { value: ' ts-secret ' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Enregistrer' }).at(-1)!);
    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('typesafe', 'ts-secret'));
    expect(mockApi.recheckNatureReading).toHaveBeenCalled();
    await waitFor(() => expect(key.value).toBe(''));
    expect(screen.queryByText(/ts-secret/)).toBeNull();
  });
});

describe('SettingsModal — Suggestions', () => {
  it('says what leaves for TypeSafe above the switches that send it', () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Propositions' }));
    expect(screen.getByText(/envoient le titre et le début des cases concernées, sans clés ni e-mails/)).toBeTruthy();
  });
});

describe('SettingsModal — AI providers', () => {
  it('enables structuring at the end of dictation by default and remembers the choice', async () => {
    localStorage.removeItem('dictation_auto_structure');
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Dictée' }));
    const option = (await screen.findByLabelText('Structurer la case à la fin de la dictée')) as HTMLInputElement;
    expect(option.checked).toBe(true);
    fireEvent.click(option);
    expect(localStorage.getItem('dictation_auto_structure')).toBe('false');
  });
  it('shows the registry providers and remembers the choice', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));

    const clean = (await screen.findByLabelText('Nettoyage de la dictée (LLM)')) as HTMLSelectElement;
    const stt = screen.getByLabelText('Transcription (voix)') as HTMLSelectElement;

    await waitFor(() => expect([...clean.options].map(o => o.value)).toEqual(['cerebras', 'ollama']));
    expect([...stt.options].map(o => o.value)).toEqual(['deepgram']);
    expect(clean.value).toBe('cerebras');

    fireEvent.change(clean, { target: { value: 'ollama' } });
    expect(localStorage.getItem('ai_clean_provider')).toBe('ollama');
    // The displayed detail comes from the registry, without any key.
    expect(screen.getByText(/aucune clé requise/)).toBeTruthy();
    expect(screen.getByText(/DEEPGRAM_API_KEY/)).toBeTruthy();
    // The key's origin is displayed, never its value.
    expect(screen.getByText(/Clé définie par variable d’environnement/)).toBeTruthy();
  });

  it('shows the key variable sent by Rust (key_env) for a provider that needs a key', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    expect(await screen.findByText(/clé : CEREBRAS_API_KEY/)).toBeTruthy();
    expect(screen.getByText(/Pas encore branchée/)).toBeTruthy();
    // Fixed-address provider: no address field.
    expect(screen.queryByLabelText('Adresse du serveur')).toBeNull();
  });

  it('saves the address of a local provider and tests the connection', async () => {
    localStorage.setItem('ai_clean_provider', 'ollama');
    mockApi.setAiUrl.mockResolvedValue('http://localhost:1234/v1/chat/completions');
    mockApi.testAiConnection.mockResolvedValue(['llama3.2', 'qwen3']);
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const address = await screen.findByLabelText('Adresse du serveur');
    expect(screen.getByText(/Adresse utilisée : http:\/\/127\.0\.0\.1:11434/)).toBeTruthy();
    fireEvent.change(address, { target: { value: 'http://localhost:1234' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer l’adresse du serveur' }));
    await waitFor(() => expect(mockApi.setAiUrl).toHaveBeenCalledWith('ollama', 'http://localhost:1234'));
    expect(await screen.findByText('Adresse enregistrée.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Tester la connexion' }));
    await waitFor(() => expect(mockApi.testAiConnection).toHaveBeenCalledWith('ollama'));
    expect(await screen.findByText('Connexion réussie : 2 modèles disponibles.')).toBeTruthy();
  });

  it('shows why the connection test failed', async () => {
    mockApi.testAiConnection.mockRejectedValue('[ai_key_missing] CEREBRAS_API_KEY manquante');
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Tester la connexion' }));
    expect(await screen.findByText('Échec de la connexion : CEREBRAS_API_KEY manquante')).toBeTruthy();
  });

  it('remembers the model per provider and passes the key to Rust', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const template = (await screen.findByLabelText('Modèle (nettoyage)')) as HTMLInputElement;

    fireEvent.change(template, { target: { value: 'gpt-oss-120b-custom' } });
    expect(localStorage.getItem('ai_model_cerebras')).toBe('gpt-oss-120b-custom');

    const key = screen.getByLabelText("Clé API (nettoyage)") as HTMLInputElement;
    fireEvent.change(key, { target: { value: 'sk-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));

    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('cerebras', 'sk-secret'));
    await screen.findByText('Clé de nettoyage enregistrée.');
    // The entered key is never kept in the webview storage.
    expect(localStorage.getItem('sk-secret')).toBeNull();
  });

  it('clears the key field after saving and on provider change', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const key = (await screen.findByLabelText("Clé API (nettoyage)")) as HTMLInputElement;

    fireEvent.change(key, { target: { value: 'sk-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));
    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('cerebras', 'sk-secret'));
    // The field is cleared after success: impossible to save this key again under another provider.
    await waitFor(() => expect(key.value).toBe(''));

    fireEvent.change(key, { target: { value: 'sk-autre' } });
    const clean = screen.getByLabelText('Nettoyage de la dictée (LLM)') as HTMLSelectElement;
    fireEvent.change(clean, { target: { value: 'ollama' } });
    expect(key.value).toBe('');
  });

  it("an empty key field does not clear the existing key", async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    await screen.findByLabelText("Clé API (nettoyage)");

    // Empty field: the "Save" button must do nothing (the "Clear" button is dedicated).
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));
    expect(mockApi.setAiKey).not.toHaveBeenCalled();
  });

  it("disables Save and Clear of the section during saving", async () => {
    let resolve!: () => void;
    mockApi.setAiKey.mockImplementationOnce(() => new Promise<void>(res => { resolve = res; }));

    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const key = (await screen.findByLabelText("Clé API (nettoyage)")) as HTMLInputElement;
    fireEvent.change(key, { target: { value: 'sk-secret' } });

    const save = screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }) as HTMLButtonElement;
    const erase = screen.getByRole('button', { name: 'Effacer la clé de nettoyage' }) as HTMLButtonElement;
    const saveStt = screen.getByRole('button', { name: 'Enregistrer la clé de transcription' }) as HTMLButtonElement;

    fireEvent.click(save);
    await waitFor(() => expect(save.disabled).toBe(true));
    // Both actions of the section are locked; the other section stays free.
    expect(erase.disabled).toBe(true);
    expect(saveStt.disabled).toBe(false);

    resolve();
    await waitFor(() => expect(save.disabled).toBe(false));
    expect(erase.disabled).toBe(false);
  });

  it("a refresh failure does not call the save into question", async () => {
    // The initial load succeeds, the refresh that follows the save fails.
    mockApi.listAiProviders
      .mockResolvedValueOnce(list)
      .mockRejectedValueOnce(new Error('network unavailable'));

    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const key = (await screen.findByLabelText("Clé API (nettoyage)")) as HTMLInputElement;
    fireEvent.change(key, { target: { value: 'sk-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));

    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('cerebras', 'sk-secret'));
    // Distinct discreet message: the key IS saved, only the display could not be refreshed.
    await screen.findByText(/Clé de nettoyage enregistrée, mais la liste des fournisseurs/);
    expect(screen.queryByText(/Échec de l’enregistrement/)).toBeNull();
    // Success does not depend on the refresh: the field is indeed cleared.
    await waitFor(() => expect(key.value).toBe(''));
  });

  it("shows the banner when the response carries config_alert (Rust name in sync)", async () => {
    // Regression N2: the Rust field is `config_alert`; a phantom `alerte_config` left the
    // banner dead (always undefined). The test fails if the name is misspelled.
    mockApi.listAiProviders.mockResolvedValue({
      ...list,
      config_alert: 'JSON invalide (ligne 1, colonne 3) ; copie conservée : ai-config.json.illisible-42',
    });

    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));

    const banner = await screen.findByRole('alert');
    expect(banner.textContent).toMatch(/Configuration des clés illisible/);
    expect(banner.textContent).toMatch(/copie conservée/);
  });
});

// Bug 2b601abe: a delayed key response cleared the field while the user had
// changed provider and typed another key. Each section ties response, field and provider
// to an input generation.
describe('SettingsModal — input generation', () => {
  it("does not clear another provider's input and does not show its stale status", async () => {
    // The response of the first save is held until the test releases it.
    let resolveAt!: () => void;
    mockApi.setAiKey.mockImplementationOnce(() => new Promise<void>(res => { resolveAt = res; }));

    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const key = (await screen.findByLabelText("Clé API (nettoyage)")) as HTMLInputElement;
    const clean = screen.getByLabelText('Nettoyage de la dictée (LLM)') as HTMLSelectElement;

    // Provider A (Cerebras): input then save in flight.
    fireEvent.change(key, { target: { value: 'CLE-A' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));
    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('cerebras', 'CLE-A'));

    // While waiting, the user switches to Ollama (B) and types its key.
    fireEvent.change(clean, { target: { value: 'ollama' } });
    fireEvent.change(key, { target: { value: 'CLE-B' } });

    // The stale response from A arrives.
    resolveAt();

    // The durable indicator is refreshed even if the user left the flow.
    await waitFor(() => expect(mockApi.listAiProviders).toHaveBeenCalledTimes(2));
    // B's input is kept and A's message is not displayed as if it concerned B.
    expect(key.value).toBe('CLE-B');
    expect(screen.queryByText(/Clé de nettoyage enregistrée/)).toBeNull();

    // B's save succeeds normally: field cleared and status displayed.
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));
    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('ollama', 'CLE-B'));
    await screen.findByText('Clé de nettoyage enregistrée.');
    await waitFor(() => expect(key.value).toBe(''));
  });

  it('keeps an extended input for the same provider and shows the status', async () => {
    let resolve!: () => void;
    mockApi.setAiKey.mockImplementationOnce(() => new Promise<void>(res => { resolve = res; }));

    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));
    const key = (await screen.findByLabelText("Clé API (nettoyage)")) as HTMLInputElement;

    fireEvent.change(key, { target: { value: 'CLE-A1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé de nettoyage' }));
    await waitFor(() => expect(mockApi.setAiKey).toHaveBeenCalledWith('cerebras', 'CLE-A1'));

    // The user extends their input while waiting, without changing provider:
    // the generation stays active, the status is displayed and the new input is not overwritten.
    fireEvent.change(key, { target: { value: 'CLE-A12' } });
    resolve();

    await screen.findByText('Clé de nettoyage enregistrée.');
    expect(key.value).toBe('CLE-A12');
  });
});

describe('SettingsModal — full archive', () => {
  it("does not call Rust until the preparation is ready", async () => {
    let release!: (value: { ready: true } | { ready: false; reason: string }) => void;
    const prepareBackup = vi.fn(() => new Promise<{ ready: true } | { ready: false; reason: string }>(resolve => {
      release = resolve;
    }));
    render(<SettingsModal onClose={() => {}} prepareBackup={prepareBackup} />);
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));
    fireEvent.click(screen.getByRole('button', { name: 'Sauvegarder cases et images' }));
    expect(prepareBackup).toHaveBeenCalledOnce();
    expect(mockApi.createBackupArchive).not.toHaveBeenCalled();
    release({ ready: false, reason: 'Une dictée est active.' });
    await screen.findByText(/Sauvegarde impossible : Une dictée est active/);
    expect(mockApi.createBackupArchive).not.toHaveBeenCalled();
  });

  it('creates a timestamped archive containing notes and images, then shows its path', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));

    const button = screen.getByRole('button', { name: 'Sauvegarder cases et images' });
    expect(screen.getByText(/les cases et les images, pas les clés IA/)).toBeTruthy();

    fireEvent.click(button);

    await waitFor(() =>
      expect(mockApi.createBackupArchive).toHaveBeenCalledWith(
        expect.stringMatching(/^humcase-\d{4}-\d{2}-\d{2}-\d{4}\.humcase\.zip$/)
      )
    );
    await screen.findByText(/Archive créée : C:\\Users\\Alex\\Documents\\Humcase Backups/);
  });

  it("shows the creation failure and re-enables the button", async () => {
    mockApi.createBackupArchive.mockRejectedValueOnce(new Error('disque plein'));
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));

    const button = screen.getByRole('button', { name: 'Sauvegarder cases et images' });
    fireEvent.click(button);

    await screen.findByText(/Échec de la création de l’archive : Error: disque plein/);
    expect(button.hasAttribute('disabled')).toBe(false);
  });
});

describe('SettingsModal — restore from an archive', () => {
  it('reviews the archive and shows its date, projects, notes and images', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));

    fireEvent.change(screen.getByLabelText('Restaurer depuis une archive'), { target: { value: archivePath } });
    fireEvent.click(screen.getByRole('button', { name: 'Examiner l’archive' }));

    expect(mockApi.inspectBackupArchive).toHaveBeenCalledWith(archivePath);
    await screen.findByText('Archive valide');
    expect(screen.getByText(/Créée le 23\/09\/2026/)).toBeTruthy();
    expect(screen.getByText(/3 projets · 42 cases · 7 images/)).toBeTruthy();
  });

  it('invalidates the preview as soon as the path changes', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    await openRestoreConfirmation();
    expect(screen.getByText('Archive valide')).toBeTruthy();

    fireEvent.change(screen.getByLabelText('Restaurer depuis une archive'), { target: { value: 'D:\\autre.humcase.zip' } });

    expect(screen.queryByText('Archive valide')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Restaurer cette archive' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Confirmer le remplacement complet' })).toBeNull();
  });

  it('requires the confirmation then runs both barriers with the reviewed SHA-256', async () => {
    const prepareBackup = vi.fn(async () => ({ ready: true as const }));
    render(<SettingsModal onClose={() => {}} prepareBackup={prepareBackup} />);
    await openRestoreConfirmation();

    expect(screen.getByText(/Remplace tous vos projets et cases par ceux de l’archive/)).toBeTruthy();
    expect(screen.getByText(/Vos données actuelles sont sauvegardées avant/)).toBeTruthy();
    const confirmButton = screen.getByRole('button', { name: 'Confirmer le remplacement complet' }) as HTMLButtonElement;
    expect(confirmButton.disabled).toBe(true);
    expect(mockApi.restoreBackupArchive).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    expect(confirmButton.disabled).toBe(false);
    fireEvent.click(confirmButton);

    await screen.findByText('Restauration terminée. L’application va redémarrer.');
    expect(prepareBackup).toHaveBeenCalledOnce();
    expect(mockPrepareQuickCapture).toHaveBeenCalledOnce();
    expect(mockApi.restoreBackupArchive).toHaveBeenCalledWith(archivePath, archivePreview.archive_sha256);
    expect(screen.queryByRole('button', { name: 'Confirmer le remplacement complet' })).toBeNull();
  });

  it('stops the restore if the editor flush blocks', async () => {
    const prepareBackup = vi.fn(async () => ({ ready: false as const, reason: 'Une dictée est active.' }));
    render(<SettingsModal onClose={() => {}} prepareBackup={prepareBackup} />);
    await openRestoreConfirmation();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirmer le remplacement complet' }));

    await screen.findByText('Restauration bloquée : Une dictée est active.');
    expect(mockPrepareQuickCapture).not.toHaveBeenCalled();
    expect(mockApi.restoreBackupArchive).not.toHaveBeenCalled();
  });

  it('stops the restore if Quick Capture holds a draft', async () => {
    mockPrepareQuickCapture.mockReturnValue({ ready: false, reason: 'Un brouillon de la capture rapide doit être conservé.' });
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    await openRestoreConfirmation();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirmer le remplacement complet' }));

    await screen.findByText('Restauration bloquée : Un brouillon de la capture rapide doit être conservé.');
    expect(mockApi.restoreBackupArchive).not.toHaveBeenCalled();
  });

  it('shows as is the complete message returned by Rust', async () => {
    mockApi.restoreBackupArchive.mockRejectedValueOnce(new Error('Échec de la restauration : empreinte invalide'));
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    await openRestoreConfirmation();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirmer le remplacement complet' }));

    await screen.findByText('Échec de la restauration : empreinte invalide');
    expect(screen.getByRole('button', { name: 'Confirmer le remplacement complet' })).toBeTruthy();
  });

  it('shows a blocked-restore message without a prefix', async () => {
    mockApi.restoreBackupArchive.mockRejectedValueOnce(
      new Error(
        'Restauration appliquée — la restauration a été publiée, mais le verrou de session n’a pas pu être repris. Quittez Humcase depuis l’icône de la zone de notification (Quitter), puis relancez l’application.'
      )
    );
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    await openRestoreConfirmation();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirmer le remplacement complet' }));

    // The neutral marker must NOT be preceded by "Restore failed: ".
    await screen.findByText(/^Restauration appliquée — /);
    expect(screen.queryByText(/Échec de la restauration/)).toBeNull();
    expect(screen.getByRole('button', { name: 'Confirmer le remplacement complet' })).toBeTruthy();
  });

  it('Escape closes the settings when no restore is in progress', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('prevents changing the path or closing the window during the restore', async () => {
    let finishRestore!: () => void;
    mockApi.restoreBackupArchive.mockReturnValueOnce(new Promise<void>(resolve => { finishRestore = resolve; }));
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} prepareBackup={async () => ({ ready: true })} />);
    await openRestoreConfirmation();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Je comprends que cette opération remplacera mes données locales.' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirmer le remplacement complet' }));
    await waitFor(() => expect(mockApi.restoreBackupArchive).toHaveBeenCalledOnce());

    expect((screen.getByLabelText('Restaurer depuis une archive') as HTMLInputElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Fermer les paramètres' }));
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(onClose).not.toHaveBeenCalled();

    finishRestore();
    await screen.findByText('Restauration terminée. L’application va redémarrer.');
    expect((screen.getByLabelText('Restaurer depuis une archive') as HTMLInputElement).disabled).toBe(false);
  });
});

// P2-08: the microphone is only requested on an explicit action from the Dictation section.
describe('SettingsModal — micro (P2-08)', () => {
  const stop = vi.fn();
  const getUserMedia = vi.fn();
  const enumerateDevices = vi.fn();

  beforeEach(() => {
    stop.mockClear();
    getUserMedia.mockReset();
    enumerateDevices.mockReset();
    getUserMedia.mockResolvedValue({ getTracks: () => [{ stop }] });
    enumerateDevices.mockResolvedValue([
      { kind: 'audioinput', deviceId: 'mic-1', label: 'Micro 1' },
      { kind: 'audiooutput', deviceId: 'spk-1', label: 'Haut-parleurs' },
    ]);
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia, enumerateDevices },
    });
  });

  afterEach(() => {
    delete (navigator as unknown as { mediaDevices?: unknown }).mediaDevices;
  });

  it("never calls getUserMedia when opening the settings on another section", async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);

    // Enumeration alone (without permission) is allowed; no microphone request.
    await waitFor(() => expect(enumerateDevices).toHaveBeenCalled());
    expect(getUserMedia).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Général' }));
    fireEvent.click(screen.getByRole('button', { name: 'Données' }));
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it('only requests the microphone on an explicit action from the Dictation section', async () => {
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);

    fireEvent.click(screen.getByRole('button', { name: 'Dictée' }));
    // Opening the Dictation section is not enough: no automatic request.
    expect(getUserMedia).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Détecter les microphones' }));
    await waitFor(() => expect(getUserMedia).toHaveBeenCalledWith({ audio: true }));
    // The short stream is released immediately (the microphone stays closed).
    await waitFor(() => expect(stop).toHaveBeenCalled());
    // Only microphones are offered in the selector.
    expect(screen.getByRole('option', { name: 'Micro 1' })).toBeTruthy();
    expect(screen.queryByRole('option', { name: 'Haut-parleurs' })).toBeNull();
  });

  it('does not show empty parentheses for a microphone with no name or identifier', async () => {
    enumerateDevices.mockResolvedValue([{ kind: 'audioinput', deviceId: '', label: '' }]);
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Dictée' }));
    expect(await screen.findByRole('option', { name: 'Microphone inconnu' })).toBeTruthy();
    expect(screen.queryByText(/\(\)/)).toBeNull();
  });
});

describe('SettingsModal — review 0.9.69: unreadable providers', () => {
  it('announces the read failure and rereads the list on request', async () => {
    mockApi.listAiProviders.mockRejectedValueOnce(new Error('lecture impossible'));
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Services IA' }));

    expect((await screen.findByRole('alert')).textContent).toContain('n’a pas pu être lue');
    fireEvent.click(screen.getByRole('button', { name: 'Réessayer' }));

    const clean = screen.getByLabelText('Nettoyage de la dictée (LLM)') as HTMLSelectElement;
    await waitFor(() => expect([...clean.options].map(o => o.value)).toEqual(['cerebras', 'ollama']));
    expect(screen.queryByText(/n’a pas pu être lue/)).toBeNull();
  });
});

describe('SettingsModal — agent (MCP)', () => {
  it("loads the permission when the section is opened and toggles it", async () => {
    mockApi.getMcpPermissions.mockResolvedValue({ allow_note_edit: true });
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);

    // The section is not viewed: no permission read is triggered.
    expect(mockApi.getMcpPermissions).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Agents' }));
    const toggle = (await screen.findByLabelText('Autoriser l’agent à modifier les cases')) as HTMLInputElement;
    await waitFor(() => expect(toggle.checked).toBe(true));
    expect(mockApi.getMcpPermissions).toHaveBeenCalledOnce();
    // The displayed text is frozen here literally, independently of the imported constant: reintroducing
    // a sentence that falsely widens the scope (e.g. modifiable "due date", or "every
    // modification is attributed" covering the dedicated tools) makes this test fail.
    const EXPECTED_TEXT =
      "Actif : l’agent peut modifier titre, texte, nature, tags, priorité et échéance. Chacune de ces modifications est datée, signée, et l’ancienne version gardée. Jamais la dictée brute, jamais de suppression. Même désactivé, l’agent peut changer le statut, écrire un compte rendu et poser une échéance, par ses outils dédiés.";
    expect(AGENT_PERMISSION_TEXT).toBe(EXPECTED_TEXT);
    expect(screen.getByText(EXPECTED_TEXT)).toBeTruthy();

    fireEvent.click(toggle);
    await waitFor(() => expect(mockApi.setMcpAllowNoteEdit).toHaveBeenCalledWith(false));
    await waitFor(() => expect(toggle.checked).toBe(false));
  });

  it("disables the toggle while saving then reflects the success", async () => {
    let resolve!: () => void;
    mockApi.setMcpAllowNoteEdit.mockImplementationOnce(() => new Promise<void>(res => { resolve = res; }));
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Agents' }));
    const toggle = (await screen.findByLabelText('Autoriser l’agent à modifier les cases')) as HTMLInputElement;
    await waitFor(() => expect(toggle.disabled).toBe(false));

    fireEvent.click(toggle);
    await waitFor(() => expect(toggle.disabled).toBe(true));
    resolve();
    await waitFor(() => expect(toggle.disabled).toBe(false));
    expect(toggle.checked).toBe(true);
  });

  it("shows the toggle failure and keeps the previous state", async () => {
    mockApi.setMcpAllowNoteEdit.mockRejectedValueOnce(new Error('lock busy'));
    render(<SettingsModal onClose={() => {}} prepareBackup={async () => ({ ready: true })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Agents' }));
    const toggle = (await screen.findByLabelText('Autoriser l’agent à modifier les cases')) as HTMLInputElement;
    await waitFor(() => expect(toggle.disabled).toBe(false));

    fireEvent.click(toggle);
    await screen.findByText(/Échec de l’enregistrement : Error: lock busy/);
    expect(toggle.checked).toBe(false);
  });
});

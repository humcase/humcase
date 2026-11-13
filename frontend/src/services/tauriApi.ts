import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import type { Activity, Brief, CaseEvent, CaseJournal, CaseReview, CaseContext, CasePosition, ContentKind, NoteType, Project, ProjectSpace, Note, NoteRelations, NoteRevision, CreateNoteInput, UpdateNoteInput, AiProvidersInfo, AiUsageRecord, AgentSeen, Recognition, MeaningHit, MeaningStatus, NatureReview, NatureReadingStatus, Germination, Proposal, ProposalAnswer, ProposalOutcome, TidyItem, TidyAnswer, TidyOutcome, TidySettings, Supersession, ProjectSummary, ProfileView, ProfileAnswer, ProfileOutcome, ProfileSettings, Nudge, NudgeAnswer } from '../types';

export interface BackupArchivePreview {
  archive_sha256: string;
  format_version: number;
  created_at: string;
  database_schema_version: number;
  project_count: number;
  note_count: number;
  image_count: number;
  total_uncompressed_bytes: number;
}

/** MCP permissions visible to the interface (flags only, no key). */
export interface McpPermissions {
  allow_note_edit: boolean;
}

export const api = {
  getActivities: async (): Promise<Activity[]> => await invoke('get_activities'),
  createActivity: async (name: string, space: ProjectSpace): Promise<Activity> => await invoke('create_activity', { name, space }),
  renameActivity: async (id: string, name: string): Promise<Activity> => await invoke('rename_activity', { id, name }),
  deleteActivity: async (id: string): Promise<void> => { await invoke('delete_activity', { id }); },
  setProjectActivities: async (projectId: string, activityIds: string[], expectedActivityIds?: string[]): Promise<void> => { await invoke('set_project_activities', { projectId, activityIds, expectedActivityIds }); },
  getOrCreateInbox: async (space: ProjectSpace): Promise<string> => await invoke('get_or_create_inbox', { space }),
  /** Files an Inbox case into a project, inside `parentId` when given, all or nothing. */
  moveNoteToProject: async (noteId: string, projectId: string, parentId: string | null = null): Promise<void> => { await invoke('move_note_to_project', { noteId, projectId, parentId }); },
  getProjects: async (): Promise<Project[]> => {
    const projects = await invoke<Project[]>('get_projects');
    return projects.map(project => project.is_inbox ? { ...project, name: i18n.t('common.inboxOf', { space: i18n.t(project.space === 'work' ? 'common.space.work' : 'common.space.private') }) } : project);
  },
  
  createProject: async (name: string, color?: string, space?: ProjectSpace | null): Promise<string> => {
    return await invoke('create_project', { name, color, space });
  },

  updateProjectSpace: async (id: string, space: ProjectSpace | null): Promise<void> => {
    await invoke('update_project_space', { id, space });
  },
  renameProject: async (id: string, name: string): Promise<void> => { await invoke('rename_project', { id, name }); },
  setProjectColor: async (id: string, color: string): Promise<void> => { await invoke('set_project_color', { id, color }); },
  setProjectArchived: async (id: string, archived: boolean): Promise<void> => { await invoke('set_project_archived', { id, archived }); },
  /** Archives (hides) or unarchives a note; nothing is erased and the status does not change. */
  setNoteArchived: async (id: string, archived: boolean): Promise<void> => { await invoke('set_note_archived', { id, archived }); },
  /** Deletes the project; its notes move to the Inbox of its space. Returns how many notes moved. */
  deleteProject: async (id: string): Promise<number> => await invoke('delete_project', { id }),

  /** Fingerprint of the notes state: changes when a note is created, deleted or written, including by an agent. */
  notesChangeMarker: async (): Promise<string> => await invoke('notes_change_marker'),

  getNotes: async (projectId?: string, status?: string, searchQuery?: string): Promise<Note[]> => {
    return await invoke('get_notes', { projectId, status, searchQuery });
  },

  getNote: async (id: string): Promise<Note> => {
    return await invoke('get_note_by_id', { id });
  },

  createNote: async (input: CreateNoteInput): Promise<string> => {
    return await invoke('create_note', { input });
  },

  updateNote: async (id: string, input: UpdateNoteInput): Promise<void> => {
    return await invoke('update_note', { id, input });
  },

  retryNoteUpdate: async (id: string, input: UpdateNoteInput, base: Partial<UpdateNoteInput>): Promise<'applied' | 'already_applied' | 'conflict' | 'not_found'> => {
    return await invoke('retry_note_update', { id, input, base });
  },

  /**
   * Appends text to a note's content as a single **atomic write** (append in the database). Used
   * when the target note is no longer displayed: without read-modify-write, a concurrent write
   * (MCP agent) cannot be overwritten. `autoTitle` is only applied if the title is still empty
   * or "Nouvelle note" (Rust side).
   */
  appendNoteContent: async (id: string, text: string, autoTitle?: string): Promise<void> => {
    await invoke('append_note_content', { id, text, autoTitle });
  },

  appendRawContent: async (id: string, text: string): Promise<void> => {
    await invoke('append_raw_content', { id, text });
  },

  /** Moves the note to the trash (restorable for 30 days). */
  /** Places a case in the trash only if it is still empty; says whether it went. */
  trashIfEmpty: (id: string): Promise<boolean> => invoke<boolean>('trash_if_empty', { id }),
  deleteNote: async (id: string): Promise<void> => {
    await invoke('delete_note', { id });
  },

  getTrashedNotes: async (): Promise<Note[]> => {
    return await invoke('get_trashed_notes');
  },

  /** Takes a note out of the trash, unchanged. */
  restoreFromTrash: async (id: string): Promise<void> => {
    await invoke('restore_from_trash', { id });
  },

  /** Permanently erases a note from the trash. */
  purgeNote: async (id: string): Promise<void> => {
    await invoke('purge_note', { id });
  },

  /** Empties the trash; returns the number of notes erased. */
  /** Erases the trashed cases the person was shown; one trashed since stays. */
  emptyTrash: async (ids: string[]): Promise<number> => {
    return await invoke('empty_trash', { ids });
  },

  getNoteRelations: async (noteId: string): Promise<NoteRelations> => {
    return await invoke('get_note_relations', { noteId });
  },

  /** Current MCP permissions (flags only). */
  getMcpPermissions: async (): Promise<McpPermissions> => {
    return await invoke('get_mcp_permissions');
  },

  /** Turns on or off note editing by an MCP agent (flag on the Rust side). */
  setMcpAllowNoteEdit: async (enabled: boolean): Promise<void> => {
    await invoke('set_mcp_allow_note_edit', { enabled });
  },

  /** Attributed history of a note's edits, from most recent to oldest. */
  /** Agent writes recorded since `since` (ISO 8601 UTC), across all notes. */
  listAgentRevisionsSince: async (since: string): Promise<NoteRevision[]> => {
    return await invoke('list_agent_revisions_since', { since });
  },
  getNoteHistory: async (noteId: string): Promise<NoteRevision[]> => {
    return await invoke('get_note_history', { noteId });
  },

  /** Writes of agents and of the CLI since `since` (ISO 8601 UTC), oldest first (v24 journal). */
  listAgentEventsSince: async (since: string): Promise<CaseEvent[]> => await invoke('list_agent_events_since', { since }),
  /** What changed since the person's last visit: results to review, agent writes, reworks. */
  getBrief: async (): Promise<Brief> => await invoke('get_brief'),
  /** "Mark all as seen": the brief starts again from now. */
  markBriefSeen: async (): Promise<string> => await invoke('mark_brief_seen'),
  /** The person opened a case: the brief stops listing what happened on it before. */
  markCaseSeen: async (noteId: string): Promise<void> => { await invoke('mark_case_seen', { noteId }); },
  /** Who did what on a case, the decisions on its result and where they stand. */
  getCaseJournal: async (noteId: string): Promise<CaseJournal> => await invoke('get_case_journal', { noteId }),
  /** Accepts the reviewed revision of a result, or asks for a rework with a reason. */
  reviewCase: async (noteId: string, decision: CaseReview['decision'], expectedRevision: number, comment?: string): Promise<CaseReview> =>
    await invoke('review_case', { noteId, decision, expectedRevision, comment: comment ?? null }),

  /** Path and provenance of a case. */
  getCaseContext: async (id: string): Promise<CaseContext> => await invoke('get_case_context', { id }),
  /** Creates a text sub-case inside `parentId`; returns its identifier. */
  /** Creates a sub-case; a sketch created without content starts as an empty drawing. */
  createSubCase: async (parentId: string, title: string, noteType?: NoteType, contentKind?: ContentKind, content?: string): Promise<string> =>
    await invoke('create_sub_case', { parentId, title, noteType, contentKind, content }),
  /** The board of `caseId` (the top-level board of `projectId` without it) as a JSON Canvas document. */
  exportJsonCanvas: async (projectId: string, caseId?: string): Promise<string> => await invoke('export_json_canvas', { projectId, caseId }),
  /** Creates the cases of a JSON Canvas document, all or none, inside `parentId` or at the top of `projectId`. */
  importJsonCanvas: async (canvas: string, projectId?: string, parentId?: string): Promise<{ cases: string[]; ignored_edges: number }> =>
    await invoke('import_json_canvas', { projectId, parentId, canvas }),
  /** Opens the address of a link case in the default browser (http and https only). */
  openLink: async (url: string): Promise<void> => { await invoke('open_link', { url }); },
  /** Moves a case inside `parentId`, or back to top level with `null`. */
  moveCase: async (id: string, parentId: string | null): Promise<void> => { await invoke('move_case', { id, parentId }); },

  /** Positions of the cases placed on a board, in one project or in all of them. */
  getCasePositions: async (projectId?: string): Promise<CasePosition[]> => await invoke('get_case_positions', { projectId }),
  /** Places a case on its parent's board. */
  setCasePosition: async (id: string, x: number, y: number): Promise<void> => { await invoke('set_case_position', { id, x, y }); },
  setCasePositions: async (positions: CasePosition[]): Promise<void> => { await invoke('set_case_positions', { positions }); },

  createLinkedTask: async (sourceNoteId: string, title: string, content?: string): Promise<string> => {
    return await invoke('create_linked_task', { sourceNoteId, title, content });
  },

  linkNoteTask: async (sourceNoteId: string, taskNoteId: string): Promise<void> => {
    await invoke('link_note_task', { sourceNoteId, taskNoteId });
  },

  unlinkNoteTask: async (sourceNoteId: string, taskNoteId: string): Promise<void> => {
    await invoke('unlink_note_task', { sourceNoteId, taskNoteId });
  },

  /** Dictation cleanup through the chosen provider, on the Rust side: the key never goes through the webview. */
  cleanTranscript: async (transcript: string, provider?: string, model?: string, noteId?: string): Promise<string> => {
    return await invoke('clean_transcript', { transcript, provider, model, noteId });
  },

  /** Proposal without writing: only the text of this note is sent to the chosen provider. */
  structureNote: async (noteId: string, expectedContent: string, provider?: string, model?: string, frame?: string): Promise<string> => {
    return await invoke('structure_note', { noteId, expectedContent, provider, model, frame });
  },

  /** Smart dictation: the whole note with the dictation integrated, structured and corrected (no write). */
  integrateDictation: async (noteId: string, expectedContent: string, transcript: string, provider?: string, model?: string, frame?: string): Promise<string> => {
    return await invoke('integrate_dictation', { noteId, expectedContent, transcript, provider, model, frame });
  },

  /** Applies the proposal only if the source has not changed; archives the original atomically. */
  applyStructuredNote: async (noteId: string, expectedContent: string, content: string): Promise<Note> => {
    return await invoke('apply_structured_note', { noteId, expectedContent, content });
  },

  /** List of available AI providers (metadata only, no key). */
  listAiProviders: async (): Promise<AiProvidersInfo> => {
    return await invoke('list_ai_providers');
  },

  /** Saves (or clears if `apiKey` is empty) a provider's key, on the Rust side. Never read back. */
  setAiKey: async (provider: string, apiKey: string): Promise<void> => {
    await invoke('set_ai_key', { provider, apiKey });
  },

  /** Saves (or clears if empty) the server address of a local or custom provider; returns the normalized address. */
  setAiUrl: async (provider: string, url: string): Promise<string> => {
    return await invoke('set_ai_url', { provider, url });
  },

  /** Connection test: lists the provider's models without sending any content. */
  testAiConnection: async (provider: string): Promise<string[]> => {
    return await invoke('test_ai_connection', { provider });
  },

  /**
   * Opens the transcription session on the Rust side (the transcription key never leaves the Rust
   * process), in the language of the dictation (`multi`: several languages).
   */
  startTranscription: async (provider?: string, language?: string): Promise<void> => {
    await invoke('start_transcription', { provider, language });
  },

  /** Sends an audio fragment (webm/opus) to the transcription proxy. */
  sendAudioChunk: async (bytes: Uint8Array | number[]): Promise<void> => {
    await invoke('send_audio_chunk', { bytes });
  },

  /** Requests a clean close (`CloseStream`) of the transcription session. */
  stopTranscription: async (): Promise<void> => {
    await invoke('stop_transcription');
  },


  getDailyStandup: async (projectId: string): Promise<Note[]> => {
    return await invoke('get_daily_standup', { projectId });
  },

  /** Markdown resume brief assembled from the database ("Handoff" button and the MCP tool of the same name). */
  getHandoff: async (projectId?: string): Promise<string> => {
    return await invoke('get_handoff', { projectId });
  },

  findSimilarNotes: async (query: string, excludeId?: string): Promise<Note[]> => {
    return await invoke('find_similar_notes', { query, excludeId });
  },

  /** Nature and line of work read from the wording, without AI. */
  recognizeCase: async (text: string): Promise<Recognition> => {
    return await invoke('recognize_case', { text });
  },

  /** Search by meaning: where the reading of the cases stands. */
  meaningStatus: async (): Promise<MeaningStatus> => {
    return await invoke('meaning_status');
  },

  setMeaningEnabled: async (enabled: boolean): Promise<void> => {
    return await invoke('set_meaning_enabled', { enabled });
  },

  /** Cases closest in meaning to `query`, best first (empty when search by meaning is off). */
  searchByMeaning: async (query: string, limit?: number): Promise<MeaningHit[]> => {
    return await invoke('search_by_meaning', { query, limit });
  },

  /** Natures to check, the surest first: what Jev already read (nothing is sent). */
  /** Natures to check in `space` (every space when null), the surest first. */
  natureReview: async (limit?: number, space?: ProjectSpace | null): Promise<NatureReview> => {
    return await invoke('nature_review', { limit, space: space ?? null });
  },

  /** The person's answer: change to `proposed`, or keep `reviewed`. False if the case changed meanwhile. */
  answerNature: async (noteId: string, reviewed: NoteType, proposed: NoteType, change: boolean): Promise<boolean> => {
    return await invoke('answer_nature', { noteId, reviewed, proposed, change });
  },

  /** Faire germer: the AI proposes the next steps of a case, kept as proposals. */
  germinateCase: async (noteId: string, provider?: string, model?: string): Promise<Germination> => {
    return await invoke('germinate_case', { noteId, provider, model });
  },

  /** Proposals of a case waiting for an answer (Humcase's and the agents'). */
  listProposals: async (noteId: string): Promise<Proposal[]> => {
    return await invoke('list_proposals', { noteId });
  },

  answerProposals: async (noteId: string, answers: ProposalAnswer[]): Promise<ProposalOutcome> => {
    return await invoke('answer_proposals', { noteId, answers });
  },

  /** Takes answers back, right after answering. */
  undoProposals: async (noteId: string, ids: string[]): Promise<void> => {
    return await invoke('undo_proposals', { noteId, ids });
  },

  /** "To tidy": cases against a decision, decisions replaced, cases to file together, cases to archive; `refresh` looks again first. */
  tidyList: async (space: ProjectSpace | null, refresh = false): Promise<TidyItem[]> => {
    return await invoke('tidy_list', { space, refresh });
  },

  /** The check when a decision is written: the older decisions it may replace. */
  tidyCheckDecision: async (noteId: string): Promise<Supersession> => {
    return await invoke('tidy_check_decision', { noteId });
  },

  answerTidy: async (answers: TidyAnswer[]): Promise<TidyOutcome> => {
    return await invoke('answer_tidy', { answers });
  },

  /** Takes "To tidy" answers back, right after answering. */
  undoTidy: async (ids: string[]): Promise<void> => {
    return await invoke('undo_tidy', { ids });
  },

  /** A decision's place in the history: replaced by, replaces, and what it asks. */
  caseSupersession: async (noteId: string): Promise<Supersession> => {
    return await invoke('case_supersession', { noteId });
  },

  /** The person says a decision marked as replaced still holds. */
  keepDecisionInForce: async (noteId: string): Promise<void> => {
    return await invoke('keep_decision_in_force', { noteId });
  },

  /** The decisions a case may go against (the band of its page). */
  caseConflicts: async (noteId: string): Promise<TidyItem[]> => {
    return await invoke('case_conflicts', { noteId });
  },

  /** A project's summary: decisions in force, in progress, open questions, what to check. */
  projectSummary: async (projectId: string): Promise<ProjectSummary> => {
    return await invoke('project_summary', { projectId });
  },

  supersededIds: async (): Promise<string[]> => {
    return await invoke('superseded_ids');
  },

  tidySettings: async (): Promise<TidySettings> => {
    return await invoke('tidy_settings');
  },

  setTidySettings: async (settings: TidySettings): Promise<TidySettings> => {
    return await invoke('set_tidy_settings', { settings });
  },

  /** The work profile of a space and the lines proposed for it; `refresh` looks for new ones first. */
  workProfile: async (space: ProjectSpace, refresh = false): Promise<ProfileView> => {
    return await invoke('work_profile', { space, refresh });
  },

  /** The profile case of a space, created if needed (to edit it). */
  openWorkProfile: async (space: ProjectSpace): Promise<string> => {
    return await invoke('open_work_profile', { space });
  },

  answerProfile: async (answers: ProfileAnswer[]): Promise<ProfileOutcome> => {
    return await invoke('answer_profile', { answers });
  },

  undoProfile: async (ids: string[]): Promise<void> => {
    return await invoke('undo_profile', { ids });
  },

  /** The cases waiting for a next step (chosen once a day). */
  nudges: async (): Promise<Nudge[]> => {
    return await invoke('nudges');
  },

  answerNudge: async (id: string, answer: NudgeAnswer): Promise<void> => {
    return await invoke('answer_nudge', { id, answer });
  },

  profileSettings: async (): Promise<ProfileSettings> => {
    return await invoke('profile_settings');
  },

  setProfileSettings: async (settings: ProfileSettings): Promise<ProfileSettings> => {
    return await invoke('set_profile_settings', { settings });
  },

  natureReadingStatus: async (): Promise<NatureReadingStatus> => {
    return await invoke('nature_reading_status');
  },

  setNatureReading: async (enabled: boolean): Promise<void> => {
    return await invoke('set_nature_reading', { enabled });
  },

  /** A TypeSafe key was saved or cleared: the reader looks again at once. */
  recheckNatureReading: async (): Promise<void> => {
    return await invoke('recheck_nature_reading');
  },

  /** AI calls and dictations recorded since this instant (quantities only, never a price). */
  readAiUsage: async (sinceMs: number): Promise<AiUsageRecord[]> => {
    return await invoke('read_ai_usage', { sinceMs });
  },

  /** Agents that opened a session with this profile, each once, most recent first. */
  listAgentsSeen: async (): Promise<AgentSeen[]> => await invoke('list_agents_seen'),
  /** Full path of the running Humcase executable, for the commands that connect an agent. */
  humcaseExecutablePath: async (): Promise<string> => await invoke('humcase_executable_path'),

  openDataDir: async (): Promise<string> => {
    return await invoke('open_data_dir');
  },

  backupDatabase: async (fileName: string): Promise<string> => {
    return await invoke('backup_database', { fileName });
  },

  /** Creates a full archive of the database and images, without including the local AI keys. */
  createBackupArchive: async (fileName: string): Promise<string> => {
    return await invoke('create_backup_archive', { fileName });
  },

  /** Inspects the archive and returns its metadata before any restore. */
  inspectBackupArchive: async (path: string): Promise<BackupArchivePreview> => {
    return await invoke('inspect_backup_archive', { path });
  },

  /** Replaces the local data after checking that the archive has not changed. */
  restoreBackupArchive: async (path: string, expectedSha256: string): Promise<void> => {
    await invoke('restore_backup_archive', { path, expectedSha256 });
  },

  /** Saves a pasted image (compressed) and returns its portable reference `humcase-image://<name>`. */
  saveImageToDisk: async (bytes: Uint8Array | number[]): Promise<string> => {
    return await invoke('save_image_to_disk', { bytes });
  },
  /** Local images folder, used to display portable references. */
  getImagesDir: async (): Promise<string> => await invoke('get_images_dir'),
};

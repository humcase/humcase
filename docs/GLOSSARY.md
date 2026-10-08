# Interface glossary (i18n)

Rule for every Humcase translation: **one term = a single translation per language**, everywhere (application, website, help, support). English is the reference language and the pivot of translations; French is provided as context (original intent). Languages and method are described below.

## Method

1. **Extraction**: the current French texts go as they are into `frontend/src/i18n/locales/fr/` (no visible change); English is written at the same time, **in native English** (not word for word), with the terms below.
2. **"Vocabulary" pass** (done in 0.9.88): term changes in French ("Target French" column) and the term "case", in the language files only. Exception: the English engine texts (`en/engine.json`) keep "note", because they also serve the agent protocol (MCP, CLI).
3. **Translations** (es, de, pt-BR, it), **postponed to the production web v1** (decision of 29/09), from the final English, with this glossary and French as context; marked "beta" until native review.

## Terms

| Current French | Target French | English | Remark |
|---|---|---|---|
| Humcase | Humcase | Humcase | Never translated. |
| note (the container) | **case** (feminine: « une case », « Nouvelle case », « 283 cases ») | **case** (“New case”, “283 cases”) | Chosen on 29/09: in French, the box; in English, the folder you follow; like Hum**case**. Applied in the "vocabulary" pass; what a case is goes by its nature (see "Nature" below). |
| Réception | Réception | Inbox | |
| Espace Pro / Privé | Pro / Privé | Work / Personal | "Private" sounds "secret". |
| Projet | Projet | Project | |
| Activité | Activité | Area | "Activity" evokes a log. |
| Pensée / Idée / Tâche / Incident… (natures) | idem | Thought / Idea / Task / Issue… | Full list under "Nature (of a case)" below; "Bug" too developer-oriented, "Incident" everywhere in French since 0.9.88. |
| À faire / En cours / Résolu | idem | To do / In progress / Done | |
| Urgent · P0, Haute · P1, Normale · P2, Basse · P3 | idem | Urgent · P0, High · P1, Normal · P2, Low · P3 | |
| Échéance | Échéance | Due date | |
| Compte rendu | Compte rendu | Report | What an agent or a person hands back. |
| Résultat déclaré | Résultat déclaré | Declared result | A case an agent set to Done: a declaration, not your decision. |
| Accepter (un résultat) | Accepter | Accept | Your decision on the reviewed version. |
| Reprise (demander une reprise, reprise n°2) | Reprise | Rework (ask for a rework, rework #2) | Sent back with a reason; numbered per case. |
| Depuis votre dernière visite | idem | Since your last visit | The home brief of agent writes and results. |
| Raccourcis (Décisions, Tâches, Documents, Dessins…) | idem | Shortcuts (Decisions, Tasks, Documents, Drawings…) | Views under "All cases" gathering every case of one nature across the space; a shortcut moves nothing. "Modifier la liste" / "Edit the list" chooses the ones shown. |
| Historique (d'une case) | Historique | History | Who did what on a case, you and the agents, then its versions that can be restored. Not "Activité", which names an area. |
| Tâche liée | Tâche liée | Linked task | |
| Structurer | Structurer | Structure | |
| Faire germer | Faire germer | Sprout | Humcase proposes a case's next steps as sub-cases, each quoting its passage; the person answers. Not Structurer, which rewrites the note itself. "Existe déjà" / "Already exists": a case that may already cover a step; "Ranger ici" / "Put it here". |
| Réfléchir avec Humcase | Réfléchir avec Humcase | Think it through | The second mode of the panel: Humcase restates, links what the cases say ("Va dans ce sens" / "Goes this way", "Déjà décidé" / "Already decided", "À concilier" / "To reconcile"), proposes "Une piste" / "A lead" and asks one question. "Synthèse proposée" / "Proposed summary": corrected, then kept as a sub-case. |
| Demander à Humcase | Demander à Humcase | Ask Humcase | A question answered from the cases, each sentence with its source (the case and the sentences it cites). "Garder comme case" / "Keep as a case": the answer filed in the Inbox. "Cases les plus proches" / "Closest cases": what shows without an AI. |
| À ranger | À ranger | To tidy | What Humcase proposes to put away (archive a case resolved long ago, mark a replaced decision, file close cases together) and the cases that may go against a decision; the person answers, nothing is deleted. |
| Regrouper | Regrouper | File together | File a case inside a close case of the same project (To tidy); the direction can be swapped. |
| Va contre une décision | Va contre une décision | Goes against a decision | A case that plans or states the opposite of a decision in force; the person corrects it or says it does not. |
| Synthèse | Synthèse | Summary | A project's page of decisions in force, work in progress, open questions and what to check. |
| Décision en vigueur | Décision en vigueur | Decision in force | A live decision no newer decision replaced. |
| Profil de travail | Votre façon de travailler | Work profile (How you work) | One case per space saying how the person works (rules, lines of work, what they often refuse); agents read it. |
| Ce qui attend une suite | Ce qui attend une suite | What waits for a next step | Home's daily suggestions: open questions, ideas and decisions to take left unchanged for days. |
| Remplacée par | Remplacée par | Replaced by | A decision a newer one replaces: kept readable as history ("Historique" / "History"); "Elle reste en vigueur" / "It still holds" takes the mark back. |
| Mémoire | Mémoire | Memory | Settings section of To tidy (archive delay, comparing decisions, filing close cases together). |
| Trame (de structuration) | Trame | Frame | Not to be confused with Modèle. |
| Modèle (de note) | Modèle | Template | |
| Dictée intelligente / brute | Dictée / Dictée brute (button: « Dicter ») | Dictation / Raw dictation (button: “Dictate”) | “Smart” dropped on 08/10 (docs/UI_WRITING.md, rule 4). |
| Capture rapide | Capture rapide | Quick capture | |
| Brief Handoff | Brief de session | Session brief | "Reprise" is kept for a rework. The agent protocol keeps "handoff" (`get_handoff`). |
| Daily Standup | Point du jour | Daily recap | |
| Reprendre le fil | Reprendre le fil | Pick up where you left off | Home title. |
| Votre journée | Votre journée | Your day | |
| Aujourd’hui | Aujourd’hui | Today | The cases to follow up across the space, sub-cases included: in progress, overdue or due today, or P0/P1. Same rule as Home's "À poursuivre" / "To follow up". |
| Filtres : Rechercher, statut, nature, Inclure les sous-cases, Effacer les filtres | idem | Search, status, nature, Include sub-cases, Clear filters | One filter bar for the four views of the cases page. |
| Retour : {page} | idem | Back to {page} | Names the page a case was opened from. |
| À classer | À classer | Unsorted | |
| Corbeille / Archiver | idem | Trash / Archive | |
| Mettre à la corbeille / Effacer définitivement | idem | Move to the trash / Permanently erase | The only two words for removing a case: it goes to the trash (restorable 30 days), then is erased from the trash. "Supprimer" is kept for a project, whose cases are not erased. |
| Cases non résolues (nombres de la barre latérale) | idem | Unresolved cases (sidebar counts) | Every count of the sidebar (projects, Inbox, Today, shortcuts) counts the live cases not resolved, sub-cases included; the tooltip says so. |
| Mode Mini (Post-it épinglé) | Mode mini (épinglé) | Mini mode (pinned) | "Post-it" is a 3M brand: never used. |
| Plan de travail / Cartes / Liste / Tableau | idem | Canvas / Cards / List / Board | View modes of the cases page; the canvas is the default. |
| Palette de commandes | Palette de commandes | Command menu | |
| Paramètres | Paramètres | Settings | |
| Agent | Agent | Agent | |

### Canvas terms

The canvas, sub-cases and the document overview are in the interface. Task cases are still to come.

| Target French | English | Remark |
|---|---|---|
| Plan de travail | Canvas | Infinite surface on which cases are placed; each case has its own canvas of sub-cases. |
| Sous-case | Sub-case | A case contained in another (strict tree at first). |
| Case-tâche | Task case | A sub-case that **is** an existing linked note. |
| Aperçu document | Document overview | Organized rendering of a case and its sub-cases. |
| Nature (d'une case) | Nature (of a case) | What a case is: Pensée / Thought (`note`), Idée / Idea, Tâche / Task, Question / Question, Recherche / Research, Décision / Decision, Incident / Issue (`bug`). |
| Groupe | Group | A case with sub-cases, shown open on the canvas. |
| Regrouper | Group | Put the selected cases inside a new case. |
| Tout voir | Show all | Zoom that fits the whole canvas. |
| Chercher dans le plan | Search the canvas | |
| Texte / Dessin / Image / Lien | Text / Drawing / Image / Link | What a new sub-case holds, chosen on its draft card. A drawing is made with Excalidraw; the "Dessins" / "Drawings" shortcut gathers them. |
| Exporter / Importer | Export / Import | A board to and from a JSON Canvas file (`.canvas`), an open format read by other note-taking and whiteboard apps. |

## Other languages (beta)

Spanish, German, Brazilian Portuguese and Italian, chosen at the first translation (07/10/2026) and to be confirmed by a native review ([TRANSLATION](TRANSLATION.md)). The engine's protocol texts say "nota" / "Notiz" where the English says "note".

| English | Spanish | German | Brazilian Portuguese | Italian |
|---|---|---|---|---|
| case / sub-case | caso / subcaso | Fall / Unterfall | caso / subcaso | caso / sottocaso |
| Inbox | Bandeja de entrada | Eingang | Caixa de entrada | Inbox |
| Work / Personal (space) | Trabajo / Personal | Arbeit / Privat (Raum) | Trabalho / Pessoal | Lavoro / Personale |
| Project / Area | Proyecto / Área | Projekt / Bereich | Projeto / Área | Progetto / Area |
| Thought / Idea / Task / Question / Research / Decision / Issue | Pensamiento / Idea / Tarea / Pregunta / Investigación / Decisión / Incidencia | Gedanke / Idee / Aufgabe / Frage / Recherche / Entscheidung / Problem | Pensamento / Ideia / Tarefa / Pergunta / Pesquisa / Decisão / Problema | Pensiero / Idea / Attività / Domanda / Ricerca / Decisione / Problema |
| Nature | Naturaleza | Art | Natureza | Natura |
| To do / In progress / Done | Por hacer / En curso / Hecho | Offen / In Arbeit / Erledigt | A fazer / Em andamento / Concluído | Da fare / In corso / Fatto |
| Due date | Fecha límite | Fälligkeit | Prazo | Scadenza |
| Report | Informe | Bericht | Relatório | Resoconto |
| Declared result / Accept | Resultado declarado / Aceptar | Gemeldetes Ergebnis / Annehmen | Resultado declarado / Aceitar | Risultato dichiarato / Accetta |
| Rework | Corrección | Überarbeitung | Retrabalho | Rifacimento |
| Shortcuts / History | Atajos / Historial | Schnellzugriffe / Verlauf | Atalhos / Histórico | Scorciatoie / Cronologia |
| Linked task | Tarea vinculada | Verknüpfte Aufgabe | Tarefa vinculada | Attività collegata |
| Structure / Sprout | Estructurar / Germinar | Strukturieren / Entfalten | Estruturar / Germinar | Struttura / Fai germogliare |
| Ask Humcase / Keep as a case | Preguntar a Humcase / Guardar como caso | Humcase fragen / Als Fall behalten | Perguntar ao Humcase / Guardar como caso | Chiedi a Humcase / Tieni come caso |
| Think it through / Proposed summary | Reflexionar / Síntesis propuesta | Durchdenken / Vorgeschlagene Zusammenfassung | Refletir / Síntese proposta | Riflettere / Sintesi proposta |
| To tidy / File together | Por ordenar / Reunir | Aufräumen / Zusammenlegen | A arrumar / Reunir | Da riordinare / Riunisci |
| Goes against a decision | Va contra una decisión | Widerspricht einer Entscheidung | Vai contra uma decisão | Va contro una decisione |
| Summary / Decision in force / Replaced by | Síntesis / Decisión vigente / Reemplazada por | Zusammenfassung / Gültige Entscheidung / Ersetzt durch | Síntese / Decisão em vigor / Substituída por | Riepilogo / Decisione in vigore / Sostituita da |
| Work profile (How you work) | Perfil de trabajo (Cómo trabajas) | Deine Arbeitsweise | Seu jeito de trabalhar | Come lavori |
| Memory / Frame / Template | Memoria / Pauta / Plantilla | Gedächtnis / Gerüst / Vorlage | Memória / Esquema / Modelo | Memoria / Traccia / Modello |
| Dictation / Raw dictation | Dictado / en bruto | Diktat / Rohdiktat | Ditado / bruto | Dettatura / grezza |
| Quick capture | Captura rápida | Schnellerfassung | Captura rápida | Cattura rapida |
| Session brief / Daily recap | Brief de sesión / Resumen del día | Sitzungsbrief / Tagesüberblick | Briefing da sessão / Resumo do dia | Brief di sessione / Riepilogo del giorno |
| Pick up where you left off / Your day / Today | Retoma donde lo dejaste / Tu día / Hoy | Mach weiter, wo du warst / Dein Tag / Heute | Retome de onde parou / Seu dia / Hoje | Riprendi da dove avevi lasciato / La tua giornata / Oggi |
| Unsorted | Sin clasificar | Unsortiert | Sem classificação | Da smistare |
| Trash / Archive | Papelera / Archivar | Papierkorb / Archiv | Lixeira / Arquivar | Cestino / Archivia |
| Move to the trash / Permanently erase | Enviar a la papelera / Borrar definitivamente | In den Papierkorb / Endgültig löschen | Mover para a lixeira / Apagar definitivamente | Sposta nel cestino / Cancella definitivamente |
| Mini mode (pinned) | Modo mini (fijado) | Mini-Modus (angeheftet) | Modo mini (fixado) | Modalità mini (fissata) |
| Canvas / Cards / List / Board | Lienzo / Tarjetas / Lista / Tablero | Arbeitsfläche / Karten / Liste / Board | Plano / Cartões / Lista / Quadro | Lavagna / Schede / Elenco / Bacheca |
| Command menu / Settings / Agent | Menú de comandos / Ajustes / Agente | Befehlsmenü / Einstellungen / Agent | Menu de comandos / Configurações / Agente | Menu dei comandi / Impostazioni / Agente |
| Document overview | Vista de documento | Dokumentansicht | Visão em documento | Anteprima documento |

## Tone and address

| Language | Address |
|---|---|
| French | « vous » |
| English | direct and simple, short sentences |
| German | « du » |
| Spanish | « tú » |
| Italian | « tu » |
| Brazilian Portuguese | « você » |

The small labels of the "game object" universe (monospaced capitals: VOTRE JOURNÉE, TRACES…) require short translations; German serves as the length test. Quotation marks: « » in French, “ ” in English, each language's own conventions elsewhere.

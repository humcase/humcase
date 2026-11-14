// Texts of the website in this language (beta: translated from English, not reviewed yet).
// Same keys as fr.js; npm run check-i18n compares them.
export const DATES = { months: { Sep: 'Sep.', Oct: 'Okt.' }, since: 'seit {d}. {m}', done: 'erledigt am {d}. {m}', day: '{d}. {m}{t}' };

export const EL = {
    'Download for Windows': 'Für Windows herunterladen',
    'Watch one idea <em>grow</em>.': 'Sieh zu, wie eine Idee <em>wächst</em>.',
    'It starts <em>bare</em>.': 'Sie beginnt <em>kahl</em>.',
    'Then it <em>grows</em>.': 'Dann <em>wächst</em> sie.',
    'Old leaves fall, and <em>stay</em>.': 'Alte Blätter fallen, aber sie <em>bleiben</em>.',
    'Then it <em>flowers</em> again.': 'Dann <em>blüht</em> sie wieder.',
    'Your <em>turn</em>.': 'Jetzt bist <em>du</em> dran.',
    'Press <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd> to add a case · <kbd>Enter</kbd> suggests next steps · <kbd>Esc</kbd> starts over':
      'Drück <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd>, um einen Fall hinzuzufügen · <kbd>Eingabe</kbd> schlägt nächste Schritte vor · <kbd>Esc</kbd> beginnt neu',
    'Work with your <em>agents</em>.': 'Arbeite mit deinen <em>Agents</em>.',
    'Inside a <em>case</em>.': 'Im Inneren eines <em>Falls</em>.',
    '<span class="mono">Home · example data</span>Your day at a glance: what you changed, what your agents wrote, what waits for you.':
      '<span class="mono">Start · Beispieldaten</span>Dein Tag auf einen Blick: was du geändert hast, was deine Agents geschrieben haben, was auf dich wartet.',
    '<span class="mono">Canvas · example data</span>A case and its sub-cases, laid out by hand.':
      '<span class="mono">Arbeitsfläche · Beispieldaten</span>Ein Fall und seine Unterfälle, von Hand angeordnet.',
    '<span class="mono">A result from an agent · example data</span>Claude Code hands in a comparison. You accept it, or ask for a rework.':
      '<span class="mono">Ein Ergebnis von einem Agent · Beispieldaten</span>Claude Code liefert einen Vergleich ab. Du nimmst ihn an oder forderst eine Überarbeitung.',
    'Your work stays <em>yours</em>.': 'Deine Arbeit bleibt <em>deine</em>.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>. Installing with <code>winget install Humcase</code> avoids the warning.':
      'Das Installationsprogramm ist noch nicht signiert, deshalb zeigt Microsoft Defender SmartScreen eine Warnung. Klick auf <kbd>Weitere Informationen</kbd> und dann auf <kbd>Trotzdem ausführen</kbd>. Mit <code>winget install Humcase</code> erscheint die Warnung nicht.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>.':
      'Das Installationsprogramm ist noch nicht signiert, deshalb zeigt Microsoft Defender SmartScreen eine Warnung. Klick auf <kbd>Weitere Informationen</kbd> und dann auf <kbd>Trotzdem ausführen</kbd>.',
    'Every decision has <em>roots</em>.': 'Jede Entscheidung hat ihre <em>Wurzeln</em>.',
    'Why <em>Humcase</em>?': 'Warum <em>Humcase</em>?',
    'Plant your first <em>idea</em>.': 'Pflanz deine erste <em>Idee</em>.',
    'Result ready, waiting for your decision<small>Codex cannot accept it for you</small>': 'Ergebnis fertig, wartet auf deine Entscheidung<small>Codex kann es nicht für dich annehmen</small>',
    '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · open source under AGPL-3.0': '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · Open Source unter AGPL-3.0',
  };

export const T = {
    'Contact': 'Kontakt', 'Legal notice': 'Impressum',
    'Terms': 'Nutzungsbedingungen',
    'Agents': 'Agents', 'Questions': 'Fragen', 'Question': 'Frage',
    // navigation, chapters, buttons
    'The cycle': 'Der Zyklus', 'The app': 'Die App', 'Privacy': 'Datenschutz', 'Download': 'Herunterladen',
    'Humcase on GitHub': 'Humcase auf GitHub',
    'Bare': 'Kahl', 'Grows': 'Wächst', 'Falls': 'Fällt', 'Flowers': 'Blüte',
    'Idea': 'Idee', 'Task': 'Aufgabe', 'Decision': 'Entscheidung', 'Research': 'Recherche',
    'Enter · Suggest': 'Eingabe · Vorschlag', 'Esc · Start over': 'Esc · Neu beginnen',
    'Replay': 'Nochmal', 'Accept': 'Annehmen', 'Ask for a rework': 'Überarbeitung anfordern', 'Keep': 'Behalten', 'Refuse': 'Ablehnen',
    'Get release news': 'Neuigkeiten erhalten', 'Email for release news': 'E-Mail für Neuigkeiten zu neuen Versionen',
    'you@example.com': 'du@beispiel.de', 'Language:': 'Sprache:', 'beta': 'Beta',
    'PS C:\\Users\\you\\shop> ': 'PS C:\\Users\\du\\shop> ',
    'Source on GitHub': 'Code auf GitHub', 'Copied': 'Kopiert', 'Select and copy': 'Markieren und kopieren',
    // hero and cycle
    'Track your ideas, tasks and decisions, and the work your agents hand in.':
      'Verfolge deine Ideen, Aufgaben und Entscheidungen und die Arbeit, die deine Agents dir übergeben.',
    'Working with agents leaves a trail of Markdown files, chats and folders. To find out why something was decided, you search, open and reread.':
      'Die Arbeit mit Agents hinterlässt eine Spur aus Markdown-Dateien, Chats und Ordnern. Um herauszufinden, warum etwas entschieden wurde, musst du suchen, öffnen und nachlesen.',
    'In Humcase, every idea, decision, task and proof sits in one place you read at a glance, next to what led to it.':
      'In Humcase steht jede Idee, Entscheidung, Aufgabe und jeder Beleg an einem Ort, auf einen Blick lesbar, neben dem, was dazu geführt hat.',
    'Free · Open source': 'Kostenlos · Open Source',
    'Our example: redesigning a client’s online shop. Each leaf is a case: an idea, a task, a question or a decision.': 'Unser Beispiel: der Relaunch des Onlineshops eines Kunden. Jedes Blatt ist ein Fall: eine Idee, eine Aufgabe, eine Frage oder eine Entscheidung.',
    'One line, typed between two meetings: redesign the online shop. That is already a case. No setup, no AI needed.':
      'Eine Zeile, zwischen zwei Meetings getippt: Onlineshop neu gestalten. Das ist schon ein Fall. Ohne Einrichtung, ganz ohne KI.',
    'Each branch is a sub-case. With an AI service, Humcase suggests next steps from your words and creates only the ones you keep. Your agents add theirs, under their own name.':
      'Jeder Zweig ist ein Unterfall. Mit einem KI-Dienst schlägt Humcase aus deinen Worten nächste Schritte vor und legt nur die an, die du behältst. Deine Agents fügen ihre hinzu, unter ihrem eigenen Namen.',
    'Point at a framed leaf to read it': 'Zeig auf ein umrahmtes Blatt, um es zu lesen', 'Tap a framed leaf to read it': 'Tipp auf ein umrahmtes Blatt, um es zu lesen',
    'When a case is done, Humcase can suggest archiving it, and waits for your yes. An archived case stays readable: months later, you still see what was decided, and why.':
      'Ist ein Fall erledigt, kann Humcase vorschlagen, ihn zu archivieren, und wartet auf dein Ja. Ein archivierter Fall bleibt lesbar: Monate später siehst du noch, was entschieden wurde und warum.',
    'New ideas grow from what the old ones taught you, and each one remembers where it came from.':
      'Neue Ideen wachsen aus dem, was dich die alten gelehrt haben, und jede weiß noch, woher sie kommt.',
    // your turn
    'Tap a key to add a case.': 'Tipp auf eine Taste, um einen Fall hinzuzufügen.', 'An example: nothing is saved.': 'Ein Beispiel: Es wird nichts gespeichert.',
    // agents
    'Claude Code, Codex and other agents connect to Humcase and write in the same cases as you, under their own name.':
      'Claude Code, Codex und andere Agents verbinden sich mit Humcase und schreiben in dieselben Fälle wie du, unter ihrem eigenen Namen.',
    'Claude Code, asked to compare payment providers, writes a research case “Compare payment providers” into the Humcase case “Redesign the online shop” through Humcase’s MCP server. It then waits for your review.':
      'Claude Code soll Zahlungsanbieter vergleichen und legt über den MCP-Server von Humcase einen Recherchefall „Zahlungsanbieter vergleichen“ im Humcase-Fall „Onlineshop neu gestalten“ an. Danach wartet er auf deine Prüfung.',
    'An agent can’t accept its own work. It hands you a result: you accept it, or ask for a rework with a reason.':
      'Ein Agent kann seine eigene Arbeit nicht annehmen. Er gibt dir ein Ergebnis: Du nimmst es an oder bittest mit einer Begründung um Überarbeitung.',
    'Humcase doesn’t run your agents. It keeps what they hand in.': 'Humcase führt deine Agents nicht aus. Es bewahrt, was sie abliefern.',
    'shop': 'shop', 'Resize the product photos': 'Produktfotos verkleinern', 'History': 'Verlauf', 'You': 'Du',
    'created this task under “Redesign the online shop”': 'hat diese Aufgabe in „Onlineshop neu gestalten“ angelegt', 'set it in progress': 'hat sie auf „In Arbeit“ gesetzt',
    'added a result: 48 photos resized, originals kept': 'hat ein Ergebnis hinzugefügt: 48 Fotos verkleinert, Originale behalten', 'Example data': 'Beispieldaten',
    'accepted the result': 'hat das Ergebnis angenommen', 'Done, accepted by you': 'Erledigt, von dir angenommen', 'the decision is in the history': 'die Entscheidung steht im Verlauf',
    'asked for a rework: “keep the white background”': 'hat eine Überarbeitung angefordert: „weißen Hintergrund behalten“',
    'Back to Codex, in progress': 'Zurück an Codex, in Arbeit', 'your reason travels with the task': 'deine Begründung bleibt an der Aufgabe', 'just now': 'gerade eben',
    'Humcase MCP server connected': 'Humcase-MCP-Server verbunden',
    'Compare three payment providers for a small shop, and file it in Humcase': 'Vergleiche drei Zahlungsanbieter für einen kleinen Shop und leg das in Humcase ab',
    'Reading the fees and docs of three providers…\n✓ 3 sources read, summary written': 'Lese Gebühren und Dokumentation von drei Anbietern …\n✓ 3 Quellen gelesen, Zusammenfassung geschrieben',
    '→ Humcase: research “Compare payment providers” added to “Redesign the online shop”': '→ Humcase: Recherche „Zahlungsanbieter vergleichen“ zu „Onlineshop neu gestalten“ hinzugefügt',
    '  It waits for your review: accept it, or ask for a rework.': '  Sie wartet auf deine Prüfung: Nimm sie an oder fordere eine Überarbeitung an.',
    // the app, privacy, questions, final call
    'Write, dictate, sketch, add images. Arrange your cases on a canvas, or see them as cards, a list or a board. Work and personal stay apart.':
      'Schreiben, diktieren, skizzieren, Bilder einfügen. Ordne deine Fälle auf einer Arbeitsfläche an oder sieh sie als Karten, Liste oder Board. Arbeit und Privates bleiben getrennt.',
    'On your computer': 'Auf deinem Computer', 'Your cases stay on your computer and work offline. Dictation and AI services with an API key need the internet.':
      'Deine Fälle bleiben auf deinem Computer und funktionieren offline. Diktat und KI-Dienste mit API-Schlüssel brauchen Internet.',
    'Licensed under AGPL-3.0. Source code on request.': 'Lizenziert unter AGPL-3.0. Quellcode auf Anfrage.',
    'Licensed under AGPL-3.0.': 'Lizenziert unter AGPL-3.0.',
    'AI when you want it': 'KI nur, wenn du willst',
    'Humcase works without AI. To add it: an API key or a local model.':
      'Humcase funktioniert ohne KI. Um sie hinzuzufügen: ein API-Schlüssel oder ein lokales Modell.',
    'Is it free?': 'Ist es kostenlos?',
    'Yes. Humcase is free and open source, under AGPL-3.0.':
      'Ja. Humcase ist kostenlos und Open Source, unter AGPL-3.0.',
    'Is it a note app or a task manager?': 'Ist es eine Notiz-App oder eine Aufgabenverwaltung?',
    'Both. A case can be an idea, a task, a question or a decision, and cases grow inside each other. Each one keeps where it came from.':
      'Beides. Ein Fall kann eine Idee, eine Aufgabe, eine Frage oder eine Entscheidung sein, und Fälle wachsen ineinander. Jeder behält, woher er kommt.',
    'Which agents work with it?': 'Mit welchen Agents funktioniert es?',
    'Claude Code, Codex, Cursor, opencode, and any agent that supports MCP. They write under their own name and can’t change your text unless you allow it.':
      'Claude Code, Codex, Cursor, opencode und jeder Agent, der MCP unterstützt. Sie schreiben unter ihrem eigenen Namen und können deinen Text nur ändern, wenn du es erlaubst.',
    'Mac or Linux?': 'Mac oder Linux?', 'Windows 10 and 11.': 'Windows 10 und 11.',
    'Windows warns me when I install it. Why?': 'Windows warnt mich bei der Installation. Warum?',
    'Only release news. Unsubscribe in one click.': 'Nur Neuigkeiten zu neuen Versionen. Abmelden mit einem Klick.',
    'Almost there: open the email we sent and confirm.': 'Fast geschafft: Öffne die E-Mail, die wir dir geschickt haben, und bestätige.',
    'This email address doesn’t look right.': 'Diese E-Mail-Adresse scheint nicht zu stimmen.',
    'That didn’t work. Try again in a moment.': 'Das hat nicht geklappt. Versuch es gleich noch einmal.',
    'The robot check couldn’t load. Turn off your blocker for humcase.com, then try again.': 'Die Robot-Prüfung konnte nicht laden. Schalte deinen Blocker für humcase.com aus und versuch es dann noch einmal.',
    'Sending…': 'Wird gesendet…',
    'The download isn’t open yet. Windows 10 and 11 · unsigned preview': 'Der Download ist noch nicht geöffnet. Windows 10 und 11 · unsignierte Vorschau',
    'Humcase: every decision has roots': 'Humcase: Jede Entscheidung hat ihre Wurzeln', 'Humcase, home': 'Humcase, Startseite', 'Site': 'Website', 'Footer': 'Fußzeile', 'The cycle of an idea': 'Der Kreislauf einer Idee',
    'Chapters of the cycle': 'Kapitel des Kreislaufs', 'Add a case to the example idea': 'Der Beispielidee einen Fall hinzufügen',
    'Example: Claude Code files research into Humcase': 'Beispiel: Claude Code legt eine Recherche in Humcase ab',
    'Example journal of a case': 'Beispiel für das Journal eines Falls', 'Language': 'Sprache',
    // the legends of the 3D keyboard's keys
    'idea': 'Idee', 'task': 'Aufgabe', 'decision': 'Entscheidung', 'research': 'Recherche', 'sprout': 'Entfalten', 'empty': 'Leeren',
    // the leaves' tags
    'archived': 'archiviert', 'Proposed by Humcase': 'Von Humcase vorgeschlagen',
    'kept by you, proposed by Humcase': 'von dir behalten, von Humcase vorgeschlagen', 'Windows 10 and 11': 'Windows 10 und 11',
    'unsigned preview': 'unsignierte Vorschau',
    'added by Claude': 'von Claude hinzugefügt', 'added by Codex': 'von Codex hinzugefügt', 'added by you': 'von dir hinzugefügt', 'open': 'offen',
    'proposed by Humcase, kept by you': 'von Humcase vorgeschlagen, von dir behalten', 'result waiting for you': 'Ergebnis wartet auf dich',
    'archived, not deleted': 'archiviert, nicht gelöscht', 'waiting for your review': 'wartet auf deine Prüfung',
    'from your own words': 'aus deinen eigenen Worten', 'not created yet': 'noch nicht angelegt',
    // the example: a client's online shop
    'Shipping options in Europe': 'Versandoptionen in Europa', 'grew from “Shipping options in Europe”': 'gewachsen aus „Versandoptionen in Europa“',
    'Who buys on mobile?': 'Wer kauft per Smartphone?', 'Rewrite the product pages': 'Produktseiten neu schreiben',
    'Checkout on a single page': 'Kasse auf einer einzigen Seite', 'Export the old catalogue': 'Alten Katalog exportieren',
    'Pick-up at the workshop': 'Abholung in der Werkstatt', 'Compare payment providers': 'Zahlungsanbieter vergleichen',
    'Gift wrapping': 'Geschenkverpackung', 'A loyalty card': 'Eine Kundenkarte', 'Free returns': 'Kostenlose Rücksendung',
    'Write the shipping page': 'Versandseite schreiben', 'Fix the size guide': 'Größentabelle korrigieren', 'Set up order emails': 'Bestell-E-Mails einrichten',
    'Ship abroad?': 'Ins Ausland liefern?', 'Keep the old links?': 'Alte Links behalten?', 'Which product goes first?': 'Welches Produkt kommt zuerst?',
    'Free shipping over €50': 'Gratisversand ab 50 €', 'Launch after the sales': 'Nach dem Schlussverkauf starten', 'Keep the same logo': 'Dasselbe Logo behalten',
    'Shops we admire': 'Shops, die wir bewundern', 'Delivery prices compared': 'Versandpreise im Vergleich', 'What customers ask most': 'Was Kunden am häufigsten fragen',
    'Test checkout on an old phone': 'Kasse auf einem alten Handy testen', 'Card only, or PayPal too?': 'Nur Karte oder auch PayPal?',
    'Three shops we like, side by side': 'Drei Shops, die uns gefallen, nebeneinander', 'Send the preview to the client': 'Vorschau an den Kunden schicken',
    'Which photo goes first?': 'Welches Foto kommt zuerst?', 'A 30-second product video': 'Ein 30-Sekunden-Produktvideo',
  };

export const ALT = {
    'Humcase home, work space: your day on a timeline of cases and agent writes, and a result from Claude Code waiting for your decision':
      'Startseite von Humcase, Bereich Arbeit: dein Tag als Zeitleiste aus Fällen und Agent-Einträgen und ein Ergebnis von Claude Code, das auf deine Entscheidung wartet',
    'The canvas of the case “Redesign the online shop”: its sub-cases laid out by hand, each with its nature and status':
      'Die Arbeitsfläche des Falls „Onlineshop neu gestalten“: seine Unterfälle von Hand angeordnet, jeder mit seiner Art und seinem Status',
    'A research case written by Claude Code comparing payment providers: a comparison table and the buttons Accept and Ask for a rework':
      'Ein von Claude Code geschriebener Recherchefall, der Zahlungsanbieter vergleicht: eine Vergleichstabelle und die Schaltflächen Annehmen und Überarbeitung anfordern',
  };

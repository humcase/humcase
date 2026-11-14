// French texts of the website. English is written in the pages and is the key: a string missing
// here stays in English. EL holds whole elements with inline markup (em, kbd, code, small),
// T plain strings (also the ones the page's scripts build), ALT the screenshots' descriptions.
/** Dates written by the page and its scripts ("since 3 Oct", "done 28 Sep", "5 Oct 09:02"). */
export const DATES = { months: { Sep: 'sept.', Oct: 'oct.' }, since: 'depuis le {d} {m}', done: 'terminée le {d} {m}', day: '{d} {m}{t}' };

export const EL = {
    'Download for Windows': 'Télécharger pour Windows',
    'Watch one idea <em>grow</em>.': 'Regardez une idée <em>pousser</em>.',
    'It starts <em>bare</em>.': 'Au début, une simple <em>tige</em>.',
    'Then it <em>grows</em>.': 'Puis elle <em>pousse</em>.',
    'Old leaves fall, and <em>stay</em>.': 'Les vieilles feuilles tombent, et <em>restent</em>.',
    'Then it <em>flowers</em> again.': 'Puis elle <em>refleurit</em>.',
    'Your <em>turn</em>.': 'À vous de <em>jouer</em>.',
    'Press <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd> to add a case · <kbd>Enter</kbd> suggests next steps · <kbd>Esc</kbd> starts over':
      'Appuyez sur <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd> pour ajouter une case · <kbd>Entrée</kbd> propose la suite · <kbd>Échap</kbd> recommence',
    'Work with your <em>agents</em>.': 'Travaillez avec vos <em>agents</em>.',
    'Inside a <em>case</em>.': 'Dans une <em>case</em>.',
    '<span class="mono">Home · example data</span>Your day at a glance: what you changed, what your agents wrote, what waits for you.':
      '<span class="mono">Accueil · données d’exemple</span>Votre journée d’un coup d’œil : ce que vous avez modifié, ce que vos agents ont écrit, ce qui vous attend.',
    '<span class="mono">Canvas · example data</span>A case and its sub-cases, laid out by hand.':
      '<span class="mono">Plan de travail · données d’exemple</span>Une case et ses sous-cases, disposées à la main.',
    '<span class="mono">A result from an agent · example data</span>Claude Code hands in a comparison. You accept it, or ask for a rework.':
      '<span class="mono">Un résultat d’agent · données d’exemple</span>Claude Code remet une comparaison. Vous l’acceptez, ou vous demandez une reprise.',
    'Your work stays <em>yours</em>.': 'Votre travail reste <em>le vôtre</em>.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>. Installing with <code>winget install Humcase</code> avoids the warning.':
      'L’installateur n’est pas encore signé : Microsoft Defender SmartScreen affiche un avertissement. Cliquez sur <kbd>Informations complémentaires</kbd>, puis sur <kbd>Exécuter quand même</kbd>. Installer avec <code>winget install Humcase</code> évite l’avertissement.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>.':
      'L’installateur n’est pas encore signé : Microsoft Defender SmartScreen affiche un avertissement. Cliquez sur <kbd>Informations complémentaires</kbd>, puis sur <kbd>Exécuter quand même</kbd>.',
    'Every decision has <em>roots</em>.': 'Chaque décision a ses <em>racines</em>.',
    'Why <em>Humcase</em>?': 'Pourquoi <em>Humcase</em> ?',
    'Plant your first <em>idea</em>.': 'Plantez votre première <em>idée</em>.',
    'Result ready, waiting for your decision<small>Codex cannot accept it for you</small>': 'Résultat prêt, il attend votre décision<small>Codex ne peut pas l’accepter à votre place</small>',
    '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · open source under AGPL-3.0': '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · open source sous AGPL-3.0',
  };

export const T = {
    'Contact': 'Contact', 'Legal notice': 'Mentions légales',
    'Terms': 'Conditions',
    // the same words in French as in English, listed so that every language translates them
    'Agents': 'Agents', 'Questions': 'Questions', 'Question': 'Question',
    // navigation, chapters, buttons
    'The cycle': 'Le cycle', 'The app': 'L’app', 'Privacy': 'Confidentialité', 'Download': 'Télécharger',
    'Humcase on GitHub': 'Humcase sur GitHub',
    'Bare': 'Tige', 'Grows': 'Pousse', 'Falls': 'Chute', 'Flowers': 'Floraison',
    'Idea': 'Idée', 'Task': 'Tâche', 'Decision': 'Décision', 'Research': 'Recherche',
    'Enter · Suggest': 'Entrée · Suite', 'Esc · Start over': 'Échap · Recommencer',
    'Replay': 'Rejouer', 'Accept': 'Accepter', 'Ask for a rework': 'Demander une reprise', 'Keep': 'Garder', 'Refuse': 'Refuser',
    'Get release news': 'Recevoir les nouveautés', 'Email for release news': 'E-mail pour les nouveautés',
    'you@example.com': 'vous@exemple.fr', 'Language:': 'Langue :', 'beta': 'bêta',
    'PS C:\\Users\\you\\shop> ': 'PS C:\\Users\\vous\\boutique> ',
    'Source on GitHub': 'Code sur GitHub', 'Copied': 'Copié', 'Select and copy': 'Sélectionnez et copiez',
    // hero and cycle
    'Track your ideas, tasks and decisions, and the work your agents hand in.':
      'Suivez vos idées, tâches et décisions, et le travail que vos agents vous remettent.',
    'Working with agents leaves a trail of Markdown files, chats and folders. To find out why something was decided, you search, open and reread.':
      'Travailler avec des agents laisse derrière soi des fichiers Markdown, des conversations et des dossiers. Pour retrouver pourquoi on a décidé quelque chose, il faut chercher, ouvrir, relire.',
    'In Humcase, every idea, decision, task and proof sits in one place you read at a glance, next to what led to it.':
      'Dans Humcase, chaque idée, décision, tâche et preuve est au même endroit, lisible d’un coup d’œil, à côté de ce qui l’a amenée.',
    'Free · Open source': 'Gratuit · Open source',
    'Our example: redesigning a client’s online shop. Each leaf is a case: an idea, a task, a question or a decision.': 'Notre exemple : la refonte de la boutique en ligne d’un client. Chaque feuille est une case : une idée, une tâche, une question ou une décision.',
    'One line, typed between two meetings: redesign the online shop. That is already a case. No setup, no AI needed.':
      'Une ligne, tapée entre deux réunions : refonte de la boutique en ligne. C’est déjà une case. Rien à configurer, pas besoin d’IA.',
    'Each branch is a sub-case. With an AI service, Humcase suggests next steps from your words and creates only the ones you keep. Your agents add theirs, under their own name.':
      'Chaque branche est une sous-case. Avec un service d’IA, Humcase propose la suite à partir de vos mots et ne crée que ce que vous gardez. Vos agents ajoutent les leurs, sous leur nom.',
    'Point at a framed leaf to read it': 'Survolez une feuille encadrée pour la lire', 'Tap a framed leaf to read it': 'Touchez une feuille encadrée pour la lire',
    'When a case is done, Humcase can suggest archiving it, and waits for your yes. An archived case stays readable: months later, you still see what was decided, and why.':
      'Quand une case est terminée, Humcase peut proposer de l’archiver, et attend votre accord. Une case archivée reste lisible : des mois plus tard, vous voyez encore ce qui a été décidé, et pourquoi.',
    'New ideas grow from what the old ones taught you, and each one remembers where it came from.':
      'Les nouvelles idées naissent de ce que les anciennes vous ont appris, et chacune garde la trace de son origine.',
    // your turn
    'Tap a key to add a case.': 'Touchez une lettre pour ajouter une case.', 'An example: nothing is saved.': 'Un exemple : rien n’est enregistré.',
    // agents
    'Claude Code, Codex and other agents connect to Humcase and write in the same cases as you, under their own name.':
      'Claude Code, Codex et d’autres agents se connectent à Humcase et écrivent dans les mêmes cases que vous, sous leur nom.',
    'Claude Code, asked to compare payment providers, writes a research case “Compare payment providers” into the Humcase case “Redesign the online shop” through Humcase’s MCP server. It then waits for your review.':
      'Claude Code, à qui l’on demande de comparer des solutions de paiement, écrit une case de recherche « Comparer les solutions de paiement » dans la case Humcase « Refonte de la boutique en ligne », par le serveur MCP de Humcase. Elle attend ensuite votre relecture.',
    'An agent can’t accept its own work. It hands you a result: you accept it, or ask for a rework with a reason.':
      'Un agent ne peut pas accepter son propre travail. Il vous rend un résultat : vous l’acceptez, ou vous demandez une reprise avec une raison.',
    'Humcase doesn’t run your agents. It keeps what they hand in.': 'Humcase ne fait pas tourner vos agents. Il garde ce qu’ils lui remettent.',
    'shop': 'boutique', 'Resize the product photos': 'Redimensionner les photos produits', 'History': 'Historique', 'You': 'Vous',
    'created this task under “Redesign the online shop”': 'a créé cette tâche dans « Refonte de la boutique en ligne »', 'set it in progress': 'l’a passée en cours',
    'added a result: 48 photos resized, originals kept': 'a ajouté un résultat : 48 photos redimensionnées, originaux gardés', 'Example data': 'Données d’exemple',
    'accepted the result': 'a accepté le résultat', 'Done, accepted by you': 'Terminé, accepté par vous', 'the decision is in the history': 'la décision est dans l’historique',
    'asked for a rework: “keep the white background”': 'a demandé une reprise : « garder le fond blanc »',
    'Back to Codex, in progress': 'Retour à Codex, en cours', 'your reason travels with the task': 'votre raison accompagne la tâche', 'just now': 'à l’instant',
    'Humcase MCP server connected': 'serveur MCP Humcase connecté',
    'Compare three payment providers for a small shop, and file it in Humcase': 'Compare trois solutions de paiement pour une petite boutique, et range ça dans Humcase',
    'Reading the fees and docs of three providers…\n✓ 3 sources read, summary written': 'Lecture des tarifs et de la doc de trois solutions…\n✓ 3 sources lues, résumé rédigé',
    '→ Humcase: research “Compare payment providers” added to “Redesign the online shop”': '→ Humcase : recherche « Comparer les solutions de paiement » ajoutée à « Refonte de la boutique en ligne »',
    '  It waits for your review: accept it, or ask for a rework.': '  Elle attend votre relecture : acceptez-la, ou demandez une reprise.',
    // the app, privacy, questions, final call
    'Write, dictate, sketch, add images. Arrange your cases on a canvas, or see them as cards, a list or a board. Work and personal stay apart.':
      'Écrivez, dictez, dessinez, ajoutez des images. Disposez vos cases sur un plan de travail, ou affichez-les en cartes, en liste ou en tableau. Le pro et le perso restent séparés.',
    'On your computer': 'Sur votre ordinateur', 'Your cases stay on your computer and work offline. Dictation and AI services with an API key need the internet.':
      'Vos cases restent sur votre ordinateur et fonctionnent hors ligne. La dictée et les services d’IA par clé API demandent une connexion.',
    'Licensed under AGPL-3.0. Source code on request.': 'Sous licence AGPL-3.0. Code source sur demande.',
    'Licensed under AGPL-3.0.': 'Sous licence AGPL-3.0.',
    'AI when you want it': 'L’IA, si vous le voulez',
    'Humcase works without AI. To add it: an API key or a local model.':
      'Humcase fonctionne sans IA. Pour l’ajouter : une clé API ou un modèle local.',
    'Is it free?': 'Est-ce gratuit ?',
    'Yes. Humcase is free and open source, under AGPL-3.0.':
      'Oui. Humcase est gratuit et open source, sous licence AGPL-3.0.',
    'Is it a note app or a task manager?': 'Est-ce une application de notes ou de tâches ?',
    'Both. A case can be an idea, a task, a question or a decision, and cases grow inside each other. Each one keeps where it came from.':
      'Les deux. Une case peut être une idée, une tâche, une question ou une décision, et les cases poussent les unes dans les autres. Chacune garde d’où elle vient.',
    'Which agents work with it?': 'Quels agents sont compatibles ?',
    'Claude Code, Codex, Cursor, opencode, and any agent that supports MCP. They write under their own name and can’t change your text unless you allow it.':
      'Claude Code, Codex, Cursor, opencode, et tout agent compatible MCP. Ils écrivent sous leur nom et ne peuvent pas modifier votre texte sans votre accord.',
    'Mac or Linux?': 'Mac ou Linux ?', 'Windows 10 and 11.': 'Windows 10 et 11.',
    'Windows warns me when I install it. Why?': 'Windows affiche un avertissement à l’installation. Pourquoi ?',
    'Only release news. Unsubscribe in one click.': 'Uniquement les nouvelles versions. Désinscription en un clic.',
    'Almost there: open the email we sent and confirm.': 'Presque fini : ouvrez l’e-mail reçu et confirmez.',
    'This email address doesn’t look right.': 'Cette adresse e-mail ne semble pas correcte.',
    'That didn’t work. Try again in a moment.': 'Ça n’a pas marché. Réessayez dans un instant.',
    'The robot check couldn’t load. Turn off your blocker for humcase.com, then try again.': 'La vérification anti-robot n’a pas pu se charger. Désactivez votre bloqueur pour humcase.com, puis réessayez.',
    'Sending…': 'Envoi…',
    'The download isn’t open yet. Windows 10 and 11 · unsigned preview': 'Le téléchargement n’est pas encore ouvert. Windows 10 et 11 · préversion non signée',
    'Humcase: every decision has roots': 'Humcase : chaque décision a ses racines', 'Humcase, home': 'Humcase, accueil', 'Site': 'Site', 'Footer': 'Pied de page', 'The cycle of an idea': 'Le cycle d’une idée',
    'Chapters of the cycle': 'Chapitres du cycle', 'Add a case to the example idea': 'Ajouter une case à l’idée d’exemple',
    'Example: Claude Code files research into Humcase': 'Exemple : Claude Code range une recherche dans Humcase',
    'Example journal of a case': 'Exemple de journal d’une case', 'Language': 'Langue',
    // the legends of the 3D keyboard's keys
    'idea': 'idée', 'task': 'tâche', 'decision': 'décision', 'research': 'recherche', 'sprout': 'suite', 'empty': 'vider',
    // the leaves' tags
    'archived': 'archivée', 'Proposed by Humcase': 'Proposée par Humcase',
    'kept by you, proposed by Humcase': 'gardée par vous, proposée par Humcase', 'Windows 10 and 11': 'Windows 10 et 11',
    'unsigned preview': 'préversion non signée',
    'added by Claude': 'ajoutée par Claude', 'added by Codex': 'ajoutée par Codex', 'added by you': 'ajoutée par vous', 'open': 'ouverte',
    'proposed by Humcase, kept by you': 'proposée par Humcase, gardée par vous', 'result waiting for you': 'résultat en attente de vous',
    'archived, not deleted': 'archivée, pas supprimée', 'waiting for your review': 'en attente de votre relecture',
    'from your own words': 'à partir de vos mots', 'not created yet': 'pas encore créée',
    // the example: a client's online shop
    'Shipping options in Europe': 'Options de livraison en Europe', 'grew from “Shipping options in Europe”': 'née de « Options de livraison en Europe »',
    'Who buys on mobile?': 'Qui achète sur mobile ?', 'Rewrite the product pages': 'Réécrire les fiches produits',
    'Checkout on a single page': 'Paiement sur une seule page', 'Export the old catalogue': 'Exporter l’ancien catalogue',
    'Pick-up at the workshop': 'Retrait à l’atelier', 'Compare payment providers': 'Comparer les solutions de paiement',
    'Gift wrapping': 'Emballage cadeau', 'A loyalty card': 'Une carte de fidélité', 'Free returns': 'Retours gratuits',
    'Write the shipping page': 'Écrire la page livraison', 'Fix the size guide': 'Corriger le guide des tailles', 'Set up order emails': 'Configurer les e-mails de commande',
    'Ship abroad?': 'Livrer à l’étranger ?', 'Keep the old links?': 'Garder les anciens liens ?', 'Which product goes first?': 'Quel produit en premier ?',
    'Free shipping over €50': 'Livraison offerte dès 50 €', 'Launch after the sales': 'Lancer après les soldes', 'Keep the same logo': 'Garder le même logo',
    'Shops we admire': 'Des boutiques qu’on admire', 'Delivery prices compared': 'Tarifs de livraison comparés', 'What customers ask most': 'Ce que les clients demandent le plus',
    'Test checkout on an old phone': 'Tester le paiement sur un vieux téléphone', 'Card only, or PayPal too?': 'Carte seule, ou PayPal aussi ?',
    'Three shops we like, side by side': 'Trois boutiques qu’on aime, côte à côte', 'Send the preview to the client': 'Envoyer l’aperçu au client',
    'Which photo goes first?': 'Quelle photo en premier ?', 'A 30-second product video': 'Une vidéo produit de 30 secondes',
  };

export const ALT = {
    'Humcase home, work space: your day on a timeline of cases and agent writes, and a result from Claude Code waiting for your decision':
      'Accueil de Humcase, espace pro : votre journée sur une frise des cases et des écritures d’agents, et un résultat de Claude Code en attente de votre décision',
    'The canvas of the case “Redesign the online shop”: its sub-cases laid out by hand, each with its nature and status':
      'Le plan de travail de la case « Refonte de la boutique en ligne » : ses sous-cases disposées à la main, chacune avec sa nature et son statut',
    'A research case written by Claude Code comparing payment providers: a comparison table and the buttons Accept and Ask for a rework':
      'Une case de recherche de Claude Code qui compare des solutions de paiement : un tableau comparatif et les boutons Accepter et Demander une reprise',
  };

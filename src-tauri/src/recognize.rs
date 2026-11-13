//! Recognition without AI: the nature of a case (task, idea, question, decision, bug, research,
//! note) and its line of work (development, marketing, research, reflection, personal) read from
//! its wording, in the six launch languages (EN, FR, ES, DE, IT, PT-BR).
//!
//! Each nature and line of work has cue phrases with a weight; the text scores the cues it holds
//! and the best one is offered only when it clearly leads. A cue is matched on whole words, in
//! lower case and without accents; a cue ending with `*` matches the start of a word. Measured on
//! a labeled set of quick notes in six languages (`recognize_cases.json`, `recognition_bench`).

use crate::models::note::NoteType;

/// A recognized value and how far it leads the others (0 to 1).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Guess<T> {
    pub value: T,
    pub confidence: f32,
}

/// What the wording of a case says, when it says it clearly.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Recognition {
    pub nature: Option<Guess<NoteType>>,
    /// `dev`, `marketing`, `research`, `reflection` or `personal`.
    pub line_of_work: Option<Guess<&'static str>>,
}

/// Least score and lead a guess needs to be offered.
const MIN_SCORE: f32 = 1.0;
const MIN_CONFIDENCE: f32 = 0.5;

type Cues = &'static [(&'static str, f32)];

#[rustfmt::skip]
const TODO: Cues = &[
    // en
    ("need to", 1.0), ("needs to", 1.0), ("have to", 1.0), ("has to", 1.0), ("must", 1.0), ("todo", 1.5),
    ("to do", 1.0), ("dont forget", 1.5), ("don t forget", 1.5), ("remember to", 1.5), ("make sure", 1.0),
    ("gotta", 1.0), ("got to", 1.0), ("to book", 1.0), ("to call", 1.0), ("to send", 1.0), ("to buy", 1.0),
    ("to fix", 0.8), ("to finish", 1.0), ("asap", 1.0), ("by friday", 0.8), ("by monday", 0.8),
    ("by tomorrow", 0.8), ("follow up", 1.0),
    // fr
    ("il faut", 1.0), ("faut que", 1.0), ("faut", 0.6), ("je dois", 1.2), ("on doit", 1.2),
    ("nous devons", 1.2), ("tu dois", 1.0), ("a faire", 1.2), ("penser a", 1.5), ("ne pas oublier", 1.5),
    ("pas oublier", 1.5), ("rappeler", 0.8), ("envoyer", 0.6), ("acheter", 0.8), ("prendre rdv", 1.2),
    ("prendre rendez vous", 1.2), ("avant vendredi", 0.8), ("d ici", 0.6), ("relancer", 1.0),
    ("terminer", 0.8), ("finir", 0.8),
    // es
    ("hay que", 1.2), ("tengo que", 1.2), ("tenemos que", 1.2), ("tienes que", 1.0), ("debo", 1.0),
    ("debemos", 1.0), ("pendiente", 1.2), ("no olvidar", 1.5), ("no olvides", 1.5), ("acordarme de", 1.5),
    ("acordarse de", 1.5), ("recordar", 1.0), ("llamar a", 0.8), ("comprar", 0.8), ("enviar", 0.6),
    ("pedir cita", 1.2), ("terminar", 0.8), ("antes del viernes", 0.8), ("para el viernes", 0.8),
    // de
    ("muss", 1.0), ("muessen", 1.0), ("musst", 1.0), ("nicht vergessen", 1.5), ("erledigen", 1.2),
    ("zu erledigen", 1.5), ("daran denken", 1.5), ("denk dran", 1.5), ("bis freitag", 0.8),
    ("bis morgen", 0.8), ("anrufen", 0.8), ("kaufen", 0.8), ("schicken", 0.6), ("termin machen", 1.2),
    ("fertig machen", 1.0), ("todo", 1.5),
    // it
    ("devo", 1.2), ("dobbiamo", 1.2), ("bisogna", 1.2), ("da fare", 1.2), ("ricordarsi di", 1.5),
    ("ricordati di", 1.5), ("ricordarmi di", 1.5), ("non dimenticare", 1.5), ("chiamare", 0.8),
    ("comprare", 0.8), ("mandare", 0.6), ("prenotare", 1.0), ("entro venerdi", 0.8), ("finire", 0.8),
    // pt
    ("preciso", 1.2), ("precisamos", 1.2), ("tenho que", 1.2), ("temos que", 1.2), ("tem que", 1.0),
    ("nao esquecer", 1.5), ("lembrar de", 1.5), ("lembrar", 0.8), ("ligar para", 0.8), ("comprar", 0.8),
    ("mandar", 0.6), ("marcar", 0.8), ("agendar", 1.0), ("ate sexta", 0.8), ("terminar", 0.8), ("fazer", 0.5),
    // more, all languages
    ("i should", 0.8), ("i really should", 1.2), ("unbedingt", 0.8), ("before the", 0.4), ("by end of", 1.0),
    ("bis ende", 1.0), ("entro fine", 1.0), ("ate o fim", 1.0), ("antes de fin", 1.0), ("avant la fin", 1.0),
    ("renew", 1.0), ("renouveler", 1.0), ("renovar", 1.0), ("verlaengern", 1.0), ("rinnovare", 1.0),
    ("relire", 0.8), ("migrar", 0.6), ("pedir", 0.6), ("togliere", 0.6), ("chiedere", 0.6), ("fechar", 0.6),
    ("ajustar", 0.6), ("prenotare", 0.6), ("reservar", 0.8), ("buchen", 1.0), ("reserver", 0.8),
];

#[rustfmt::skip]
const IDEA: Cues = &[
    // en
    ("idea", 1.5), ("what if", 1.5), ("maybe", 1.0), ("perhaps", 1.0), ("could", 0.8), ("we could", 1.2),
    ("might", 0.6), ("would be nice", 1.5), ("would be cool", 1.5), ("how about", 1.2), ("why not", 1.0),
    ("thinking of", 0.8), ("it might be worth", 1.2),
    // fr
    ("idee", 1.5), ("et si", 1.5), ("peut etre", 1.0), ("on pourrait", 1.5), ("pourrait", 0.8),
    ("ca serait bien", 1.5), ("ce serait bien", 1.5), ("serait cool", 1.5), ("pourquoi pas", 1.0),
    ("pourquoi ne pas", 1.2), ("ca vaudrait le coup", 1.2),
    // es
    ("idea", 1.5), ("y si", 1.5), ("quizas", 1.0), ("quiza", 1.0), ("tal vez", 1.0), ("podriamos", 1.5),
    ("estaria bien", 1.5), ("seria genial", 1.5), ("por que no", 1.0), ("a lo mejor", 1.0),
    // de
    ("idee", 1.5), ("was waere wenn", 1.5), ("vielleicht", 1.0), ("koennten", 1.2), ("koennte", 0.8),
    ("waere cool", 1.5), ("waere schoen", 1.5), ("waere gut", 1.2), ("warum nicht", 1.0),
    ("wie waere es", 1.2),
    // it
    ("idea", 1.5), ("e se", 1.5), ("forse", 1.0), ("potremmo", 1.5), ("potrebbe", 0.8),
    ("sarebbe bello", 1.5), ("sarebbe carino", 1.5), ("perche no", 1.0), ("magari", 1.0),
    // pt
    ("ideia", 1.5), ("e se", 1.5), ("talvez", 1.0), ("poderiamos", 1.5), ("poderia", 0.8),
    ("seria legal", 1.5), ("seria bom", 1.2), ("por que nao", 1.0), ("quem sabe", 1.0),
    // more, all languages
    ("just a thought", 1.5), ("ueberlegung", 1.0), ("gedanke", 1.0), ("einfaellt", 1.2), ("ca pourrait", 1.0),
    ("si potrebbe", 1.2), ("se podria", 1.2), ("podia", 0.8), ("da pra", 0.8), ("juste une idee", 1.5),
    ("solo una idea", 1.5), ("so uma ideia", 1.5), ("brainstorm*", 1.0), ("imagine", 0.8), ("imaginons", 1.0),
    ("imagina", 0.8), ("stell dir vor", 1.2), ("immagina", 0.8),
];

#[rustfmt::skip]
const QUESTION: Cues = &[
    // en
    ("how do", 1.0), ("how can", 1.0), ("how to", 0.8), ("why does", 1.0), ("why is", 1.0),
    ("should we", 1.2), ("should i", 1.2), ("do we", 0.8), ("is it", 0.6), ("which one", 1.0),
    ("i wonder", 1.5), ("wondering", 1.2), ("not sure", 1.0), ("question", 1.2), ("any idea", 1.0),
    // fr
    ("comment", 0.6), ("pourquoi", 0.8), ("est ce que", 1.2), ("faut il", 1.2), ("quel", 0.6),
    ("quelle", 0.6), ("je me demande", 1.5), ("on se demande", 1.5), ("pas sur", 1.0), ("question", 1.2),
    ("savoir si", 1.2), ("a voir si", 1.0),
    // es
    ("como", 0.6), ("por que", 0.8), ("deberiamos", 1.2), ("debemos", 0.4), ("cual", 0.6),
    ("me pregunto", 1.5), ("no se si", 1.2), ("no estoy seguro", 1.2), ("pregunta", 1.2),
    // de
    ("wie", 0.6), ("warum", 0.8), ("sollen wir", 1.2), ("sollten wir", 1.2), ("welche", 0.6),
    ("frage mich", 1.5), ("ich frage", 1.2), ("nicht sicher", 1.0), ("frage", 1.0), ("ob wir", 1.0),
    // it
    ("come", 0.4), ("perche", 0.6), ("dovremmo", 1.2), ("quale", 0.6), ("mi chiedo", 1.5), ("non so se", 1.2),
    ("non sono sicuro", 1.2), ("domanda", 1.2),
    // pt
    ("como", 0.6), ("por que", 0.8), ("devemos", 0.6), ("deveriamos", 1.2), ("qual", 0.6), ("sera que", 1.5),
    ("me pergunto", 1.5), ("nao sei se", 1.2), ("nao tenho certeza", 1.2), ("pergunta", 1.2),
];

#[rustfmt::skip]
const DECISION: Cues = &[
    // en
    ("decided", 1.5), ("we decided", 2.0), ("decision", 1.5), ("we ll go with", 2.0), ("going with", 1.5),
    ("we chose", 1.5), ("chose", 1.0), ("settled on", 1.5), ("final", 0.8), ("agreed", 1.2),
    ("we re keeping", 1.5), ("we keep", 1.2), ("approved", 1.2), ("we re dropping", 1.2),
    // fr
    ("decide", 1.5), ("on a decide", 2.0), ("decision", 1.5), ("on part sur", 2.0), ("on garde", 1.5),
    ("on choisit", 1.5), ("choisi", 1.0), ("valide", 1.2), ("tranche", 1.2), ("c est acte", 2.0),
    ("acte", 1.0), ("retenu", 1.2), ("on abandonne", 1.2), ("definitivement", 0.8),
    // es
    ("decidido", 1.5), ("decidimos", 2.0), ("decision", 1.5), ("vamos con", 1.5), ("nos quedamos con", 2.0),
    ("elegimos", 1.5), ("aprobado", 1.2), ("queda descartado", 1.2), ("al final", 0.8), ("definitivo", 1.0),
    // de
    ("entschieden", 1.5), ("beschlossen", 1.5), ("entscheidung", 1.5), ("wir nehmen", 1.5),
    ("festgelegt", 1.2), ("bleiben bei", 1.5), ("gewaehlt", 1.0), ("genehmigt", 1.2), ("final", 0.8),
    // it
    ("deciso", 1.5), ("abbiamo deciso", 2.0), ("decisione", 1.5), ("andiamo con", 1.5), ("scelto", 1.0),
    ("abbiamo scelto", 1.5), ("confermato", 1.2), ("approvato", 1.2), ("teniamo", 1.0),
    // pt
    ("decidimos", 2.0), ("decidido", 1.5), ("decisao", 1.5), ("vamos com", 1.5), ("escolhemos", 1.5),
    ("fechado", 1.0), ("aprovado", 1.2), ("ficamos com", 1.5), ("definido", 1.0),
    // more, all languages
    ("acordado", 1.2), ("geeinigt", 1.5), ("ficou combinado", 2.0), ("combinado", 1.0), ("resolveu", 1.2),
    ("resolvemos", 1.5), ("decidi", 1.5), ("decisione presa", 2.0), ("ci concentriamo", 1.0),
    ("we ll focus on", 1.0), ("on se concentre sur", 1.0), ("on a tranche", 2.0), ("j ai choisi", 1.5),
    ("ho scelto", 1.5), ("he elegido", 1.5), ("dafuer entschieden", 1.0), ("dagegen entschieden", 1.0),
    ("restiamo su", 1.5), ("nos quedamos", 1.5), ("we re going with", 1.0), ("decided against", 1.0),
    ("i ve decided", 1.5), ("j ai decide", 1.5), ("he decidido", 1.5), ("ho deciso", 1.5), ("eu decidi", 1.5),
    ("ich habe entschieden", 1.5),
];

#[rustfmt::skip]
const BUG: Cues = &[
    // en
    ("bug", 1.5), ("broken", 1.5), ("doesn t work", 1.5), ("doesnt work", 1.5), ("not working", 1.5),
    ("stopped working", 1.5), ("crash*", 1.5), ("error", 1.0), ("fails", 1.2), ("failing", 1.2),
    ("leak*", 1.2), ("bounced", 1.2), ("dead link", 1.5), ("won t", 0.6), ("down", 0.4), ("issue", 0.6),
    // fr
    ("bug", 1.5), ("plante", 1.5), ("marche pas", 1.5), ("marche plus", 1.5), ("fonctionne pas", 1.5),
    ("fonctionne plus", 1.5), ("erreur", 1.0), ("casse", 1.2), ("en panne", 1.5), ("fuit", 1.5),
    ("fuite", 1.2), ("bloque", 1.0), ("ne s affiche", 1.2), ("probleme", 0.6),
    // es
    ("no funciona", 1.5), ("dejo de funcionar", 1.5), ("falla", 1.2), ("fallo", 1.0), ("error", 1.0),
    ("roto", 1.2), ("rota", 1.2), ("se rompio", 1.5), ("gotea", 1.5), ("caido", 1.0), ("se cuelga", 1.5),
    ("problema", 0.6),
    // de
    ("funktioniert nicht", 1.5), ("geht nicht", 1.5), ("kaputt", 1.5), ("fehler", 1.0), ("absturz", 1.5),
    ("stuerzt ab", 1.5), ("tropft", 1.5), ("defekt", 1.5), ("haengt", 1.0), ("problem", 0.6),
    // it
    ("non funziona", 1.5), ("non va", 1.0), ("errore", 1.0), ("rotto", 1.2), ("rotta", 1.2),
    ("si blocca", 1.5), ("crasha", 1.5), ("perde", 0.8), ("guasto", 1.5), ("problema", 0.6),
    // pt
    ("nao funciona", 1.5), ("parou de funcionar", 1.5), ("quebrou", 1.5), ("quebrado", 1.2), ("erro", 1.0),
    ("travou", 1.5), ("trava", 1.0), ("vazando", 1.5), ("caiu", 1.0), ("problema", 0.6),
    // more, all languages
    ("404", 1.5), ("500s", 1.5), ("bounce*", 1.2), ("rebondi*", 1.2), ("pierde agua", 1.5),
    ("perde acqua", 1.5), ("derail*", 1.2), ("deraille", 1.2), ("aus dem ruder", 1.5),
    ("out of control", 1.2), ("fora do controle", 1.5), ("hors de controle", 1.2), ("fuori controllo", 1.2),
    ("not saving", 1.2), ("timeout", 1.0), ("vazamento", 1.2), ("goteo", 1.2), ("keeps failing", 1.5),
    ("plus rien", 0.8), ("ne marche", 1.2), ("ne fonctionne", 1.2), ("non parte", 1.2),
    ("startet nicht", 1.5), ("no arranca", 1.5), ("nao liga", 1.2), ("won t start", 1.5),
    // inability: "can't sign in", "impossible de se connecter"
    ("throws", 1.0), ("can t", 0.6), ("cannot", 0.6), ("can t sign in", 1.5), ("can t log in", 1.5),
    ("impossible de", 0.8), ("no puedo", 0.6), ("no se puede", 0.8), ("kann nicht", 0.6), ("kann mich nicht", 1.0),
    ("non riesco", 0.6), ("nao consigo", 0.6), ("nao da para", 0.6),
];

#[rustfmt::skip]
const RESEARCH: Cues = &[
    // en
    ("according to", 1.5), ("study", 1.0), ("studies", 1.0), ("paper", 1.0), ("source", 1.0),
    ("found that", 1.5), ("data shows", 1.5), ("compared", 1.0), ("comparison", 1.0), ("benchmark", 1.0),
    ("survey", 0.8), ("results", 0.6), ("percent", 0.6), ("read that", 1.2),
    // fr
    ("selon", 1.2), ("d apres", 1.2), ("etude", 1.0), ("article", 0.8), ("source", 1.0), ("compare", 0.8),
    ("comparatif", 1.2), ("resultats", 0.8), ("j ai lu", 1.2), ("enquete", 0.8), ("pour cent", 0.6),
    // es
    ("segun", 1.2), ("estudio", 1.0), ("articulo", 0.8), ("fuente", 1.0), ("comparado", 0.8),
    ("comparativa", 1.2), ("resultados", 0.8), ("lei que", 1.2), ("encuesta", 0.8), ("por ciento", 0.6),
    // de
    ("laut", 1.2), ("studie", 1.0), ("quelle", 1.0), ("verglichen", 1.0), ("vergleich", 1.0),
    ("ergebnisse", 0.8), ("gelesen", 1.0), ("umfrage", 0.8), ("prozent", 0.6),
    // it
    ("secondo", 1.2), ("studio", 1.0), ("articolo", 0.8), ("fonte", 1.0), ("confronto", 1.0),
    ("risultati", 0.8), ("ho letto", 1.2), ("sondaggio", 0.8), ("per cento", 0.6),
    // pt
    ("segundo", 0.8), ("de acordo com", 1.5), ("estudo", 1.0), ("artigo", 0.8), ("fonte", 1.0),
    ("comparado", 0.8), ("comparacao", 1.0), ("resultados", 0.8), ("li que", 1.2), ("pesquisa", 0.8),
    ("por cento", 0.6),
    // more, all languages
    ("compar*", 1.0), ("confront*", 1.0), ("vergleich*", 1.0), ("vs", 1.0), ("versus", 1.0), ("lendo", 1.0),
    ("leyendo", 1.0), ("j ai regarde", 1.0), ("i looked at", 1.0), ("i ve been reading", 1.2),
    ("estuve mirando", 1.0), ("ho guardato", 1.0), ("angeschaut", 1.0), ("dei uma olhada", 1.0),
    ("concurrents", 1.0), ("competitors", 1.0), ("competidores", 1.0), ("konkurrenz", 1.0),
    ("concorrenti", 1.0), ("concorrentes", 1.0), ("lu", 0.6), ("read", 0.6), ("letto", 0.8), ("leido", 0.8),
    ("lido", 0.8),
];

#[rustfmt::skip]
const DEV: Cues = &[
    ("code", 1.0), ("api", 1.5), ("bug", 1.2), ("deploy*", 1.5), ("server", 1.0), ("serveur", 1.0),
    ("servidor", 1.0), ("commit", 1.5), ("pr", 0.6), ("pull request", 1.5), ("merge", 1.2), ("release", 1.0),
    ("frontend", 1.5), ("backend", 1.5), ("database", 1.2), ("base de donnees", 1.2), ("base de datos", 1.2),
    ("datenbank", 1.2), ("banco de dados", 1.2), ("sql", 1.5), ("query", 1.0), ("endpoint", 1.5),
    ("staging", 1.5), ("prod", 1.0), ("production", 0.8), ("tests", 0.8), ("unit test*", 1.5), ("ci", 0.8),
    ("pipeline", 1.0), ("refactor*", 1.5), ("branch", 1.0), ("branche", 0.8), ("git", 1.5), ("npm", 1.5),
    ("docker", 1.5), ("login", 0.8), ("app", 0.6), ("crash*", 0.8), ("stack trace", 1.5),
    ("bibliotheque", 0.6), ("library", 0.8), ("framework", 1.0), ("python", 1.2), ("javascript", 1.2),
    ("typescript", 1.2), ("rust", 1.0), ("react", 1.0), ("css", 1.2), ("html", 1.2), ("json", 1.2),
    ("token", 0.8), ("cache", 0.8), ("timeout", 1.2), ("version", 0.6), ("ticket", 0.8), ("sprint", 1.0),
    ("build", 1.0), ("compil*", 1.2),
    // more, all languages
    ("redis", 1.5), ("postgres", 1.5), ("postgresql", 1.5), ("mysql", 1.5), ("sqlite", 1.5), ("vite", 1.2),
    ("webpack", 1.5), ("jest", 1.5), ("vitest", 1.5), ("s3", 1.2), ("sentry", 1.5), ("repo", 1.2),
    ("infra", 1.2), ("esm", 1.2), ("dependances", 1.2), ("dependencies", 1.2), ("dependencias", 1.2),
    ("abhaengigkeiten", 1.2), ("dipendenze", 1.2), ("auth", 1.2), ("cookie", 1.0), ("dashboard", 0.8),
    ("webhook*", 1.5), ("sandbox", 1.2), ("stripe", 1.2), ("cron", 1.2), ("bundle", 1.2),
    ("feature flag*", 1.5), ("cloudflare", 1.5), ("coverage", 1.0), ("e2e", 1.5), ("flaky", 1.2),
    ("node", 1.0), ("console log", 1.5), ("db", 1.2), ("sdk", 1.5), ("microservice*", 1.5),
    ("kubernetes", 1.5), ("aws", 1.2), ("nightly", 1.0), ("date fns", 1.5), ("prs", 1.0), ("migration", 0.6),
    ("migracion", 0.6), ("migrazione", 0.6), ("migracao", 0.6), ("logs", 0.8), ("devs", 1.0),
];

#[rustfmt::skip]
const MARKETING: Cues = &[
    ("campaign", 1.5), ("campagne", 1.5), ("campana", 1.5), ("kampagne", 1.5), ("campagna", 1.5),
    ("campanha", 1.5), ("newsletter", 1.5), ("seo", 1.5), ("landing page", 1.5), ("landing", 1.0),
    ("ads", 1.2), ("pub", 0.8), ("publicite", 1.2), ("publicidad", 1.2), ("werbung", 1.2),
    ("pubblicita", 1.2), ("publicidade", 1.2), ("audience", 1.2), ("publico", 0.8), ("zielgruppe", 1.5),
    ("pubblico", 0.8), ("cible", 1.0), ("conversion", 1.2), ("conversao", 1.2), ("brand", 1.2),
    ("marque", 0.8), ("marca", 0.8), ("marke", 0.8), ("instagram", 1.5), ("linkedin", 1.5), ("tiktok", 1.5),
    ("facebook", 1.2), ("post", 0.8), ("posts", 0.8), ("influenc*", 1.5), ("lancement", 1.0), ("launch", 1.0),
    ("lanzamiento", 1.0), ("lancio", 1.0), ("lancamento", 1.0), ("clients", 0.6), ("customers", 0.8),
    ("kunden", 0.8), ("clientes", 0.6), ("clienti", 0.6), ("leads", 1.5), ("emailing", 1.5), ("mailing", 1.2),
    ("ctr", 1.5), ("engagement", 1.2), ("followers", 1.2), ("abonnes", 1.0), ("seguidores", 1.0),
    ("hashtag*", 1.5), ("copy", 0.6), ("slogan", 1.5), ("budget pub", 1.5), ("roi", 1.0), ("salon", 0.8),
    ("webinar", 1.2), ("promo", 1.2),
    // more, all languages
    ("flyers", 1.5), ("flyer", 1.5), ("imprimeur", 1.2), ("imprenta", 1.2), ("druckerei", 1.2),
    ("tipografia", 1.2), ("trade show", 1.5), ("feria", 1.2), ("messe", 1.0), ("fiera", 1.2), ("feira", 1.2),
    ("booth", 1.2), ("agency", 1.2), ("agence", 1.0), ("agencia", 1.0), ("agentur", 1.2), ("agenzia", 1.0),
    ("redes", 0.8), ("reels", 1.5), ("podcast", 1.0), ("reichweite", 1.5), ("reach", 0.8), ("alcance", 0.8),
    ("b2b", 1.2), ("relaunch", 1.0), ("markenfarben", 1.5), ("brand colors", 1.5), ("tasso di apertura", 1.5),
    ("open rate", 1.5), ("taux d ouverture", 1.5), ("tasa de apertura", 1.5), ("oeffnungsrate", 1.5),
    ("taxa de abertura", 1.5), ("sponsor*", 1.2), ("patrocinio", 1.2), ("utm", 1.5), ("banner", 1.2),
    ("tone of voice", 1.5), ("visuels", 1.0), ("graphiste", 1.0), ("black friday", 1.2), ("insta", 1.2),
    ("social media", 1.5), ("reseaux sociaux", 1.2), ("redes sociales", 1.5), ("anzeigen", 1.2),
    ("annonces", 1.0), ("anuncios", 1.2), ("annunci", 1.2), ("testimonial*", 1.2), ("temoignages", 1.2),
    ("video", 0.6), ("videos", 0.8), ("stories", 1.0), ("cta", 1.5), ("a b test", 1.2), ("ab test", 1.2),
];

#[rustfmt::skip]
const RESEARCH_WORK: Cues = &[
    ("study", 1.0), ("etude", 1.0), ("estudio", 1.0), ("studie", 1.0), ("studio", 0.6), ("estudo", 1.0),
    ("paper", 1.2), ("article", 0.8), ("articulo", 0.8), ("artikel", 0.8), ("articolo", 0.8), ("artigo", 0.8),
    ("survey", 1.0), ("enquete", 1.0), ("encuesta", 1.0), ("umfrage", 1.0), ("sondaggio", 1.0),
    ("pesquisa", 1.0), ("interview*", 1.2), ("entretien*", 1.2), ("entrevista*", 1.2), ("intervista*", 1.2),
    ("sample", 1.2), ("echantillon", 1.2), ("muestra", 1.2), ("stichprobe", 1.5), ("campione", 1.2),
    ("amostra", 1.2), ("hypothes*", 1.5), ("hipotes*", 1.5), ("ipotes*", 1.5), ("thesis", 1.5),
    ("these", 1.0), ("tesis", 1.5), ("tesi", 1.5), ("tese", 1.5), ("these", 1.0), ("literature", 1.2),
    ("litterature", 1.2), ("literatura", 1.2), ("letteratura", 1.2), ("literatur", 1.2),
    ("bibliograph*", 1.5), ("data", 0.6), ("donnees", 0.6), ("datos", 0.6), ("daten", 0.6), ("dati", 0.6),
    ("dados", 0.6), ("statisti*", 1.0), ("methodolog*", 1.5), ("metodolog*", 1.5), ("methodik", 1.5),
    ("lab", 1.0), ("labo", 1.0), ("experiment*", 1.0), ("citation", 1.0), ("cita", 0.6), ("zitat", 1.0),
    ("peer review", 1.5), ("conference", 0.8), ("supervisor", 1.2), ("directeur de these", 1.5),
    ("doktorvater", 1.5), ("orientador", 1.5), ("relatore", 1.5), ("director de tesis", 1.5),
    ("journal", 0.8), ("revue", 0.6), ("questionnaire", 1.2), ("cuestionario", 1.2), ("fragebogen", 1.2),
    ("questionario", 1.2), ("participants", 1.0), ("participantes", 1.0), ("teilnehmer", 1.0),
    ("partecipanti", 1.0),
    // more, all languages
    ("cronbach", 1.5), ("predictors", 1.2), ("transcripts", 1.2), ("transcriptions", 1.0),
    ("transcripciones", 1.0), ("transkripte", 1.2), ("trascrizioni", 1.0), ("transcricoes", 1.0),
    ("anonymi*", 1.2), ("marco teorico", 1.5), ("theoretical framework", 1.5), ("cadre theorique", 1.5),
    ("theoretischen rahmen", 1.5), ("quadro teorico", 1.5), ("referencial teorico", 1.5), ("chapter", 0.8),
    ("chapitre", 0.8), ("capitulo", 0.8), ("kapitel", 0.8), ("capitolo", 0.8), ("zotero", 1.5),
    ("mendeley", 1.5), ("befragung", 1.2), ("dataset", 1.2), ("istat", 1.2), ("censimento", 1.2),
    ("census", 1.2), ("recensement", 1.2), ("censo", 1.2), ("zensus", 1.2), ("mann whitney", 1.5),
    ("t test", 1.5), ("regress*", 1.2), ("comite de etica", 1.5), ("ethics", 1.2), ("ethique", 1.2),
    ("ethik*", 1.2), ("comitato etico", 1.5), ("comite d ethique", 1.5), ("einwilligung", 1.2), ("prof", 0.8),
    ("scraping", 1.0), ("codebook", 1.5), ("focus group*", 1.5), ("corpus", 1.2), ("n", 0.4),
    ("p value", 1.5), ("variable*", 0.8), ("coding", 0.6), ("codage", 0.8), ("codifica", 0.8),
];

#[rustfmt::skip]
const REFLECTION: Cues = &[
    ("i think", 1.0), ("i feel", 1.2), ("i wonder", 1.0), ("what matters", 1.5), ("long term", 1.0),
    ("priorities", 1.0), ("honestly", 1.0), ("in the end", 0.8), ("my goal", 1.0), ("meaning", 1.0),
    ("je pense", 1.0), ("je sens", 1.2), ("je me demande", 1.0), ("ce qui compte", 1.5), ("long terme", 1.0),
    ("priorites", 1.0), ("honnetement", 1.0), ("au fond", 1.2), ("mon objectif", 1.0),
    ("j ai l impression", 1.5), ("creo que", 1.0), ("siento", 1.2), ("me pregunto", 1.0),
    ("lo que importa", 1.5), ("largo plazo", 1.0), ("prioridades", 1.0), ("sinceramente", 1.0),
    ("en el fondo", 1.2), ("mi objetivo", 1.0), ("ich denke", 1.0), ("ich glaube", 1.0), ("ich fuehle", 1.2),
    ("was zaehlt", 1.5), ("langfristig", 1.0), ("prioritaeten", 1.0), ("ehrlich gesagt", 1.0),
    ("im grunde", 1.2), ("mein ziel", 1.0), ("penso che", 1.0), ("sento", 1.0), ("mi chiedo", 1.0),
    ("cio che conta", 1.5), ("lungo termine", 1.0), ("priorita", 1.0), ("sinceramente", 1.0),
    ("in fondo", 1.2), ("il mio obiettivo", 1.0), ("acho que", 1.0), ("sinto", 1.2), ("me pergunto", 1.0),
    ("o que importa", 1.5), ("longo prazo", 1.0), ("prioridades", 1.0), ("no fundo", 1.2),
    ("meu objetivo", 1.0), ("career", 1.0), ("carriere", 1.0), ("carrera", 1.0), ("karriere", 1.0),
    ("carriera", 1.0), ("carreira", 1.0), ("motivation", 1.0), ("motivacao", 1.0), ("motivazione", 1.0),
    ("motivacion", 1.0), ("equilibre", 1.0), ("balance", 0.8),
    // more, all languages
    ("part time", 1.5), ("temps partiel", 1.5), ("media jornada", 1.5), ("teilzeit", 1.5),
    ("meio periodo", 1.5), ("part time", 1.5), ("promotion", 1.0), ("ascenso", 1.2), ("befoerderung", 1.2),
    ("promozione", 1.2), ("promocao", 1.2), ("manager", 0.8), ("reconversion", 1.5), ("career change", 1.5),
    ("jobwechsel", 1.5), ("cambiare lavoro", 1.5), ("mudar de carreira", 1.5), ("job", 0.6), ("travail", 0.6),
    ("trabajo", 0.6), ("arbeit", 0.6), ("lavoro", 0.6), ("trabalho", 0.6), ("deep work", 1.2),
    ("routine", 1.0), ("rutina", 1.0), ("rotina", 1.0), ("procrastin*", 1.5), ("prokrastin*", 1.5),
    ("sueldo", 1.0), ("salary", 1.0), ("salaire", 1.0), ("gehalt", 1.0), ("stipendio", 1.0), ("salario", 1.0),
    ("selbststaendigkeit", 1.5), ("freelance", 1.0), ("kuendigung", 1.2), ("kuendigen", 1.2),
    ("demission*", 1.2), ("dimissioni", 1.2), ("demissao", 1.2), ("quit", 1.0), ("sabbatical", 1.5),
    ("excedencia", 1.5), ("sabbatique", 1.5), ("angst", 1.0), ("scared", 1.0), ("peur", 1.0), ("miedo", 1.0),
    ("paura", 1.0), ("medo", 1.0), ("tired", 0.8), ("stanco", 1.0), ("cansado", 1.0), ("muede", 1.0),
    ("ruolo", 1.0), ("vale a pena", 0.8), ("je sais pas", 0.8), ("keine ahnung", 0.8), ("sei la", 1.0),
    ("boh", 0.8), ("no se", 0.6), ("nao sei", 0.6), ("non so", 0.6), ("consulting", 1.0),
    ("consultoria", 1.0), ("habits", 1.0), ("habitudes", 1.0), ("gewohnheit*", 1.0), ("abitudini", 1.0),
    ("habitos", 1.0), ("burnout", 1.5), ("journal", 0.4), ("why do i", 1.2), ("pourquoi je", 1.2),
];

#[rustfmt::skip]
const PERSONAL: Cues = &[
    ("doctor", 1.5), ("medecin", 1.5), ("medico", 1.5), ("arzt", 1.5), ("dentist*", 1.5), ("dentiste", 1.5),
    ("zahnarzt", 1.5), ("dentista", 1.5), ("kids", 1.2), ("enfants", 1.2), ("ninos", 1.2), ("hijos", 1.2),
    ("kinder", 1.2), ("bambini", 1.2), ("figli", 1.2), ("filhos", 1.2), ("criancas", 1.2), ("mom", 1.2),
    ("maman", 1.2), ("mama", 1.2), ("mamma", 1.2), ("mae", 1.2), ("rent", 1.0), ("loyer", 1.5),
    ("alquiler", 1.5), ("miete", 1.5), ("affitto", 1.5), ("aluguel", 1.5), ("groceries", 1.5),
    ("courses", 1.0), ("compra", 0.6), ("einkaufen", 1.2), ("spesa", 1.2), ("mercado", 0.8),
    ("supermarket", 1.2), ("supermarche", 1.2), ("supermercado", 1.2), ("supermarkt", 1.2),
    ("supermercato", 1.2), ("vacation", 1.5), ("vacances", 1.5), ("vacaciones", 1.5), ("urlaub", 1.5),
    ("vacanze", 1.5), ("ferias", 1.5), ("car", 0.8), ("voiture", 1.0), ("coche", 1.0), ("auto", 0.8),
    ("carro", 1.0), ("bank", 0.8), ("banque", 1.0), ("banco", 0.6), ("taxes", 1.0), ("impots", 1.5),
    ("impuestos", 1.5), ("steuer*", 1.2), ("tasse", 1.0), ("impostos", 1.5), ("birthday", 1.5),
    ("anniversaire", 1.5), ("cumpleanos", 1.5), ("geburtstag", 1.5), ("compleanno", 1.5),
    ("aniversario", 1.5), ("boiler", 1.5), ("chaudiere", 1.5), ("caldera", 1.5), ("heizung", 1.5),
    ("caldaia", 1.5), ("aquecedor", 1.5), ("apartment", 1.0), ("appartement", 1.0), ("piso", 0.8),
    ("wohnung", 1.2), ("appartamento", 1.2), ("apartamento", 1.0), ("wedding", 1.2), ("mariage", 1.2),
    ("boda", 1.2), ("hochzeit", 1.2), ("matrimonio", 1.0), ("casamento", 1.2), ("gym", 1.2),
    ("salle de sport", 1.2), ("gimnasio", 1.2), ("fitnessstudio", 1.2), ("palestra", 1.2), ("academia", 1.0),
    ("vet", 1.2), ("veterinaire", 1.5), ("veterinario", 1.5), ("tierarzt", 1.5), ("insurance", 1.0),
    ("assurance", 1.0), ("seguro", 1.0), ("versicherung", 1.0), ("assicurazione", 1.0), ("school", 0.8),
    ("ecole", 1.0), ("escuela", 1.0), ("schule", 1.0), ("scuola", 1.0), ("escola", 1.0), ("dinner", 1.0),
    ("diner", 0.8), ("cena", 0.8), ("abendessen", 1.0), ("jantar", 1.0), ("flight", 1.0), ("vol", 0.6),
    ("vuelo", 1.0), ("flug", 1.0), ("volo", 1.0), ("voo", 1.0), ("passport", 1.5), ("passeport", 1.5),
    ("pasaporte", 1.5), ("reisepass", 1.5), ("passaporto", 1.5), ("passaporte", 1.5),
    // more, all languages
    ("home insurance", 1.5), ("broadband", 1.5), ("box internet", 1.5), ("isa", 1.0), ("pension", 1.0),
    ("fontanero", 1.5), ("plombier", 1.5), ("plumber", 1.5), ("klempner", 1.5), ("idraulico", 1.5),
    ("encanador", 1.5), ("grifo", 1.2), ("robinet", 1.2), ("wasserhahn", 1.2), ("rubinetto", 1.2),
    ("torneira", 1.2), ("bombillas", 1.2), ("ampoules", 1.2), ("bulbs", 1.2), ("gluehbirnen", 1.2),
    ("lampadine", 1.2), ("lampadas", 1.2), ("viaje", 1.0), ("voyage", 1.0), ("trip", 1.0), ("reise", 1.0),
    ("viaggio", 1.0), ("viagem", 1.0), ("compania de luz", 1.5), ("electricite", 1.0), ("electricity", 1.0),
    ("strom", 1.0), ("bolletta", 1.2), ("conta de luz", 1.5), ("sofa", 1.2), ("canape", 1.2), ("divano", 1.2),
    ("ikea", 1.2), ("nebenkosten*", 1.5), ("hausverwaltung", 1.5), ("balkon", 1.2), ("balcon", 1.2),
    ("balcony", 1.2), ("balcone", 1.2), ("varanda", 1.2), ("waschmaschine", 1.5), ("lavatrice", 1.5),
    ("lave linge", 1.5), ("lavadora", 1.5), ("washing machine", 1.5), ("maquina de lavar", 1.5),
    ("bollo auto", 1.5), ("commercialista", 1.5), ("mutuo", 1.2), ("mutui", 1.2), ("mortgage", 1.5),
    ("pret immobilier", 1.5), ("hipoteca", 1.5), ("hypothek", 1.5), ("codice fiscale", 1.5),
    ("pediatra", 1.5), ("pediatre", 1.5), ("pediatrician", 1.5), ("kinderarzt", 1.5), ("natal", 1.0),
    ("noel", 1.0), ("christmas", 1.0), ("navidad", 1.0), ("weihnachten", 1.0), ("natale", 1.0),
    ("famille", 0.8), ("family", 0.8), ("familia", 0.8), ("familie", 0.8), ("famiglia", 0.8),
    ("demenagement", 1.0), ("mudanza", 1.0), ("umzug", 1.0), ("trasloco", 1.0), ("mudanca", 1.0),
    ("steuererklaerung", 1.5), ("wifi", 0.8), ("garage", 0.8), ("taller", 0.8), ("werkstatt", 1.0),
    ("meccanico", 1.0), ("oficina", 0.6), ("garagiste", 1.2),
];

/// Task verbs that open a note ("send the brief…", "relire mes notes…"), in the six languages.
#[rustfmt::skip]
const TASK_VERBS: &[&str] = &[
    // en
    "bump", "rerun", "delete", "request", "send", "call", "book", "buy", "fix", "check", "update", "review",
    "write", "finish", "prepare", "ask", "email", "pay", "renew", "cancel", "order", "schedule", "add",
    "remove", "clean", "move", "submit", "print", "pick", "get", "reply", "upload", "draft", "list", "return",
    "install", "sign", "contact", "shopping", "chase", "sort", "reschedule", "confirm",
    // fr
    "envoyer", "appeler", "acheter", "relire", "faire", "preparer", "finir", "terminer", "ecrire", "verifier",
    "mettre", "corriger", "reserver", "payer", "demander", "contacter", "prendre", "ranger", "nettoyer",
    "lister", "planifier", "renouveler", "annuler", "commander", "ajouter", "supprimer", "relancer", "rendre",
    "imprimer", "repondre", "deposer", "changer", "installer", "signer", "recoder", "publier", "courses",
    // es
    "enviar", "llamar", "comprar", "preparar", "revisar", "terminar", "escribir", "pagar", "pedir",
    "reservar", "migrar", "actualizar", "corregir", "cancelar", "renovar", "contactar", "hacer", "mandar",
    "subir", "borrar", "eliminar", "agregar", "anadir", "limpiar", "firmar", "imprimir", "responder",
    "cambiar", "instalar", "publicar",
    // it
    "togliere", "chiamare", "comprare", "mandare", "inviare", "preparare", "rivedere", "finire", "scrivere",
    "pagare", "prenotare", "chiedere", "leggere", "aggiornare", "correggere", "sistemare", "controllare",
    "annullare", "rinnovare", "fare", "aggiungere", "eliminare", "pulire", "firmare", "stampare",
    "rispondere", "cambiare", "installare", "pubblicare",
    // pt
    "ajustar", "ligar", "marcar", "agendar", "atualizar", "corrigir", "fazer", "apagar", "adicionar",
    "limpar", "assinar", "escrever", "mudar", "fechar", "parar", "conversar", "revisar", "testar",
];

/// Words that open a deadline ("before Friday", "d'ici lundi", "bis Freitag").
#[rustfmt::skip]
const DEADLINE_WORDS: &[&str] = &[
    "before", "by", "until", "till", "avant", "ici", "pour", "antes", "para", "hasta", "bis",
    "vor", "entro", "prima", "ate",
];

/// Days and near dates a deadline names.
#[rustfmt::skip]
const DEADLINE_DAYS: &[&str] = &[
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "tomorrow", "tonight",
    "eod", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche", "demain", "lunes",
    "martes", "miercoles", "jueves", "viernes", "sabado", "domingo", "manana", "montag", "dienstag",
    "mittwoch", "donnerstag", "freitag", "samstag", "sonntag", "morgen", "lunedi", "martedi", "mercoledi",
    "giovedi", "venerdi", "sabato", "domenica", "domani", "segunda", "terca", "quarta", "quinta", "sexta",
    "amanha",
];

/// Whether the text names a deadline: a deadline word followed, within two words, by a day or a
/// number.
fn has_deadline(normalized: &str) -> bool {
    let words: Vec<&str> = normalized.split_whitespace().collect();
    words.iter().enumerate().any(|(i, w)| {
        DEADLINE_WORDS.contains(w)
            && words[i + 1..].iter().take(3).any(|next| {
                DEADLINE_DAYS.contains(next) || next.starts_with(|c: char| c.is_ascii_digit())
            })
    })
}

/// Whether the text, or one of its clauses after a `;`, opens with a task verb.
fn opens_with_task_verb(text: &str) -> bool {
    text.split([';', '\n']).any(|clause| {
        normalize(&umlauts(&clause.to_lowercase()))
            .split_whitespace()
            .next()
            .is_some_and(|first| TASK_VERBS.contains(&first))
    })
}

/// Lower case without accents, every run of other characters a single space, padded with spaces
/// so that a cue matches whole words (" il faut ").
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(' ');
    let mut space = true;
    for c in text.chars().flat_map(char::to_lowercase) {
        let folded = fold(c);
        if folded.is_alphanumeric() {
            out.push(folded);
            space = false;
        } else if c == 'ß' {
            out.push_str("ss");
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    if !space {
        out.push(' ');
    }
    out
}

/// The base letter of an accented Latin letter (German umlauts become their two-letter form
/// through the cues, which spell them `ae`, `oe`, `ue`).
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' => 'o',
        'ù' | 'ú' | 'û' => 'u',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// German umlauts as the cues spell them.
fn umlauts(text: &str) -> String {
    text.replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
}

fn score(normalized: &str, cues: Cues) -> f32 {
    let mut total = 0.0;
    for &(cue, weight) in cues {
        let found = match cue.strip_suffix('*') {
            Some(stem) => normalized.contains(&format!(" {stem}")),
            None => normalized.contains(&format!(" {cue} ")),
        };
        if found {
            total += weight;
        }
    }
    total
}

/// The best of `scores` when it reaches `MIN_SCORE` and leads clearly.
fn best<T: Copy>(scores: &[(T, f32)]) -> Option<Guess<T>> {
    let total: f32 = scores.iter().map(|(_, s)| s).sum();
    let &(value, top) = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
    let confidence = top / total.max(f32::EPSILON);
    (top >= MIN_SCORE && confidence >= MIN_CONFIDENCE).then_some(Guess { value, confidence })
}

/// What in a text points to a nature, to tell the person why it is proposed.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Evidence {
    /// Words of the text, as they are written ("Décision", "il faut").
    Words { words: String },
    /// The text, or one of its clauses, opens with a task verb ("Appeler").
    TaskVerb { word: String },
    /// A deadline ("avant vendredi").
    Deadline,
    /// A question mark.
    QuestionMark,
}

fn cues_of(nature: NoteType) -> Option<Cues> {
    Some(match nature {
        NoteType::Todo => TODO,
        NoteType::Idea => IDEA,
        NoteType::Question => QUESTION,
        NoteType::Decision => DECISION,
        NoteType::Bug => BUG,
        NoteType::Research => RESEARCH,
        _ => return None,
    })
}

/// Why `text` reads as `nature`: its heaviest cue, in the words of the text, otherwise a task
/// verb, a deadline or a question mark. `None` when nothing in the wording points to it.
pub fn evidence(text: &str, nature: NoteType) -> Option<Evidence> {
    let lowered = umlauts(&text.to_lowercase());
    let normalized = normalize(&lowered);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    // The same words as written: `normalize` keeps letters and digits and splits on the rest,
    // so the n-th word of both lists is the same word.
    let written: Vec<&str> = text
        .split(|c: char| !(c.is_alphanumeric() || c == 'ß'))
        .filter(|w| !w.is_empty())
        .collect();
    let aligned = written.len() == words.len();
    // Words inside a set phrase that is not a cue ("open source") never count.
    let mut excluded = vec![false; words.len()];
    for phrase in NOT_CUES {
        let phrase: Vec<&str> = phrase.split_whitespace().collect();
        for start in 0..words.len().saturating_sub(phrase.len() - 1) {
            if words[start..start + phrase.len()] == phrase[..] {
                excluded[start..start + phrase.len()].fill(true);
            }
        }
    }
    let mut best: Option<(f32, usize, usize)> = None;
    for &(cue, weight) in cues_of(nature)? {
        let (stem, prefix) = match cue.strip_suffix('*') {
            Some(stem) => (stem, true),
            None => (cue, false),
        };
        let cue_words: Vec<&str> = stem.split(' ').collect();
        let n = cue_words.len();
        let found = (0..words.len().saturating_sub(n - 1)).find(|&start| {
            !excluded[start..start + n].contains(&true)
                && cue_words.iter().enumerate().all(|(k, w)| {
                    let word = words[start + k];
                    if prefix && k == n - 1 {
                        word.starts_with(w)
                    } else {
                        word == *w
                    }
                })
        });
        if let Some(start) = found {
            if best.is_none_or(|(w, _, _)| weight > w) {
                best = Some((weight, start, n));
            }
        }
    }
    if let Some((_, start, n)) = best {
        let words = if aligned {
            written[start..start + n].join(" ")
        } else {
            words[start..start + n].join(" ")
        };
        return Some(Evidence::Words { words });
    }
    match nature {
        NoteType::Todo if opens_with_task_verb(text) => {
            let word = text
                .split([';', '\n'])
                .find_map(|clause| {
                    let first = normalize(&umlauts(&clause.to_lowercase()));
                    let first = first.split_whitespace().next()?.to_string();
                    TASK_VERBS.contains(&first.as_str()).then(|| {
                        clause
                            .split(|c: char| !c.is_alphanumeric())
                            .find(|w| !w.is_empty())
                            .unwrap_or_default()
                            .to_string()
                    })
                })
                .unwrap_or_default();
            Some(Evidence::TaskVerb { word })
        }
        NoteType::Todo if has_deadline(&normalized) => Some(Evidence::Deadline),
        NoteType::Question if text.contains(['?', '¿']) => Some(Evidence::QuestionMark),
        _ => None,
    }
}

/// Set phrases holding a cue word that do not carry its meaning: "open source" is a licence, not
/// a source one cites; "source code" is code. Removed before the cues are read.
const NOT_CUES: &[&str] = &[
    " open source ",
    " source code ",
    " source codes ",
    " code source ",
    " codes sources ",
    " source map ",
    " source maps ",
];

/// Reads the nature and the line of work of a text. Each is `None` when the wording does not
/// say it clearly.
pub fn recognize(text: &str) -> Recognition {
    let mut normalized = normalize(&umlauts(&text.to_lowercase()));
    for phrase in NOT_CUES {
        while normalized.contains(phrase) {
            normalized = normalized.replace(phrase, " ");
        }
    }
    let questions = text.matches(['?', '¿']).count() as f32;

    let mut idea = score(&normalized, IDEA);
    let mut decision = score(&normalized, DECISION);
    let asking = score(&normalized, QUESTION);
    // A hedged decision ("maybe we should decide") is not a decision yet, nor one that is asked
    // ("est-ce qu'on garde Jest ou…").
    if idea > 0.0 && decision > 0.0 {
        decision *= 0.5;
        idea += 0.5;
    }
    if asking >= 1.2 {
        decision *= 0.5;
    }
    // A question mark after a proposal ("maybe a diary study?") still proposes.
    let mark = if idea >= 1.0 { 0.75 } else { 1.5 };
    let mut todo = score(&normalized, TODO);
    if opens_with_task_verb(text) {
        todo += 1.2;
    }
    if has_deadline(&normalized) {
        todo += 1.0;
    }
    let natures = [
        (NoteType::Todo, todo),
        (NoteType::Idea, idea),
        (NoteType::Question, asking + mark * questions.min(2.0)),
        (NoteType::Decision, decision),
        (NoteType::Bug, score(&normalized, BUG)),
        (NoteType::Research, score(&normalized, RESEARCH)),
    ];
    let lines = [
        ("dev", score(&normalized, DEV)),
        ("marketing", score(&normalized, MARKETING)),
        ("research", score(&normalized, RESEARCH_WORK)),
        ("reflection", score(&normalized, REFLECTION)),
        ("personal", score(&normalized, PERSONAL)),
    ];
    Recognition {
        nature: best(&natures),
        line_of_work: best(&lines),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nature(text: &str) -> Option<NoteType> {
        recognize(text).nature.map(|g| g.value)
    }

    #[test]
    fn whole_words_without_accents() {
        assert_eq!(normalize("Il FAUT l'Idée !"), " il faut l idee ");
        // "faire" inside "affaire" is not a cue: whole words only.
        assert_eq!(nature("une affaire de famille"), None);
    }

    #[test]
    fn reads_each_nature_in_several_languages() {
        assert_eq!(
            nature("ne pas oublier d'appeler le plombier"),
            Some(NoteType::Todo)
        );
        assert_eq!(
            nature("hay que mandar la factura antes del viernes"),
            Some(NoteType::Todo)
        );
        assert_eq!(
            nature("what if we offered a free trial"),
            Some(NoteType::Idea)
        );
        assert_eq!(
            nature("und was wäre wenn wir den Newsletter wöchentlich machen"),
            Some(NoteType::Idea)
        );
        assert_eq!(
            nature("sollen wir Postgres oder SQLite nehmen?"),
            Some(NoteType::Question)
        );
        assert_eq!(
            nature("abbiamo deciso: andiamo con il piano annuale"),
            Some(NoteType::Decision)
        );
        assert_eq!(
            nature("o login não funciona desde ontem"),
            Some(NoteType::Bug)
        );
        assert_eq!(
            nature("selon l'étude de 2024, 40 % des gens lisent sur mobile"),
            Some(NoteType::Research)
        );
    }

    #[test]
    fn open_source_and_source_code_are_not_research() {
        assert_ne!(
            nature("publier le projet en open source sur GitHub"),
            Some(NoteType::Research)
        );
        assert_ne!(
            nature("read the source code of the parser"),
            Some(NoteType::Research)
        );
        assert_ne!(
            nature("lire le code source du parseur"),
            Some(NoteType::Research)
        );
        // A source one cites still counts.
        assert_eq!(
            nature("selon l'étude de 2024, 40 % des gens lisent sur mobile (source : Pew)"),
            Some(NoteType::Research)
        );
    }

    #[test]
    fn evidence_quotes_the_words_as_written() {
        assert_eq!(
            evidence("Pour la v1, on garde SQLite", NoteType::Decision),
            Some(Evidence::Words {
                words: "on garde".into()
            })
        );
        assert_eq!(
            evidence("Décision prise hier soir", NoteType::Decision),
            Some(Evidence::Words {
                words: "Décision".into()
            })
        );
        assert_eq!(
            evidence("Appeler le plombier", NoteType::Todo),
            Some(Evidence::TaskVerb {
                word: "Appeler".into()
            })
        );
        assert_eq!(
            evidence("Postgres ou SQLite ?", NoteType::Question),
            Some(Evidence::QuestionMark)
        );
        // "open source" is not a source one cites.
        assert_eq!(evidence("publier en open source", NoteType::Research), None);
        assert_eq!(evidence("une pensée", NoteType::Note), None);
    }

    #[test]
    fn a_hedged_decision_is_an_idea() {
        assert_eq!(
            nature("maybe we should decide on the pricing"),
            Some(NoteType::Idea)
        );
    }

    #[test]
    fn says_nothing_without_a_clear_lead() {
        assert_eq!(recognize("réunion avec Paul").nature, None);
        assert_eq!(recognize("").line_of_work, None);
    }

    /// A labeled quick note of `recognize_cases.json`.
    #[derive(serde::Deserialize)]
    struct Case {
        id: String,
        lang: String,
        split: String,
        line_of_work: String,
        nature: String,
        text: String,
    }

    fn cases() -> Vec<Case> {
        serde_json::from_str(include_str!("recognize_cases.json")).unwrap()
    }

    /// The editor's former rule (French and English substrings), for comparison.
    fn former_rule(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
        if has(&["bug", "crash", "erreur", "error"]) {
            Some("bug")
        } else if has(&["todo", "faire", "ajouter"]) {
            Some("todo")
        } else if has(&["idée", "idea", "pourrait"]) {
            Some("idea")
        } else {
            None
        }
    }

    fn nature_name(n: NoteType) -> &'static str {
        match n {
            NoteType::Bug => "bug",
            NoteType::Todo => "todo",
            NoteType::Idea => "idea",
            NoteType::Note => "note",
            NoteType::Question => "question",
            NoteType::Research => "research",
            NoteType::Decision => "decision",
        }
    }

    /// Offers, right offers and right answers (no offer is right for a plain note).
    #[derive(Default)]
    struct Tally {
        total: usize,
        offered: usize,
        right_offers: usize,
        right: usize,
    }

    impl Tally {
        fn add(&mut self, guess: Option<&str>, label: &str) {
            self.total += 1;
            match guess {
                Some(g) => {
                    self.offered += 1;
                    if g == label {
                        self.right_offers += 1;
                        self.right += 1;
                    }
                }
                None if label == "note" => self.right += 1,
                None => {}
            }
        }

        fn line(&self) -> String {
            format!(
                "right {:>3}/{:<3} ({:>3.0} %)  offered {:>3}  right when offered {:>3.0} %",
                self.right,
                self.total,
                100.0 * self.right as f64 / self.total.max(1) as f64,
                self.offered,
                100.0 * self.right_offers as f64 / self.offered.max(1) as f64
            )
        }
    }

    /// Floors on the whole labeled set, so that a cue change cannot quietly make the offers worse.
    /// Measured when set: nature 81 % right, 90 % right when offered; line of work 71 % right,
    /// 96 % right when offered.
    #[test]
    fn recognition_keeps_its_measured_quality() {
        let (mut nature, mut line) = (Tally::default(), Tally::default());
        for case in cases() {
            let r = recognize(&case.text);
            nature.add(r.nature.map(|g| nature_name(g.value)), &case.nature);
            line.add(r.line_of_work.map(|g| g.value), &case.line_of_work);
        }
        let share = |a: usize, b: usize| a as f64 / b.max(1) as f64;
        assert!(
            share(nature.right, nature.total) >= 0.78,
            "nature {}",
            nature.line()
        );
        assert!(
            share(nature.right_offers, nature.offered) >= 0.88,
            "nature {}",
            nature.line()
        );
        assert!(
            share(line.right, line.total) >= 0.68,
            "line {}",
            line.line()
        );
        assert!(
            share(line.right_offers, line.offered) >= 0.94,
            "line {}",
            line.line()
        );
    }

    /// Prints how the recognition does on one half of the labeled set (`RECOGNIZE_SPLIT`: `tune`,
    /// the default, `test` or `all`), per language, nature and line of work, against the former
    /// rule. `RECOGNIZE_FILE` reads another set of the same shape (a local export, never committed);
    /// `RECOGNIZE_HEAD=n` reads only the first n words; `RECOGNIZE_MISSES=1` (or `line`) lists misses;
    /// `RECOGNIZE_DUMP=path` writes the guesses.
    /// `cargo test --lib recognition_bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn recognition_bench() {
        let split = std::env::var("RECOGNIZE_SPLIT").unwrap_or_else(|_| "tune".into());
        let mut by: std::collections::BTreeMap<String, Tally> = Default::default();
        let mut misses = Vec::new();
        let mut guesses = serde_json::Map::new();
        // RECOGNIZE_FILE: another labeled set in the same shape (a local export, never committed).
        let set: Vec<Case> = match std::env::var("RECOGNIZE_FILE") {
            Ok(path) => serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap(),
            Err(_) => cases(),
        };
        for case in set.iter().filter(|c| split == "all" || c.split == split) {
            let head: Option<usize> = std::env::var("RECOGNIZE_HEAD")
                .ok()
                .and_then(|n| n.parse().ok());
            let text = match head {
                Some(n) => case
                    .text
                    .split_whitespace()
                    .take(n)
                    .collect::<Vec<_>>()
                    .join(" "),
                None => case.text.clone(),
            };
            let r = recognize(&text);
            let nature = r.nature.as_ref().map(|g| nature_name(g.value));
            let line = r.line_of_work.as_ref().map(|g| g.value);
            guesses.insert(
                case.id.clone(),
                serde_json::json!({ "nature": nature, "line": line }),
            );
            for key in [
                "nature all".to_string(),
                format!("nature {}", case.lang),
                format!("nature ={}", case.nature),
            ] {
                by.entry(key).or_default().add(nature, &case.nature);
            }
            by.entry("former rule".into())
                .or_default()
                .add(former_rule(&case.text), &case.nature);
            for key in [
                "line all".to_string(),
                format!("line {}", case.lang),
                format!("line ={}", case.line_of_work),
            ] {
                by.entry(key).or_default().add(line, &case.line_of_work);
            }
            if std::env::var("RECOGNIZE_MISSES").as_deref() == Ok("line") {
                if line != Some(case.line_of_work.as_str()) {
                    misses.push(format!(
                        "{} {} -> {:?}: {}",
                        case.id,
                        case.line_of_work,
                        line,
                        case.text.chars().take(90).collect::<String>()
                    ));
                }
            } else if nature.is_some_and(|n| n != case.nature)
                || (nature.is_none() && case.nature != "note")
            {
                misses.push(format!(
                    "{} {} -> {:?}: {}",
                    case.id,
                    case.nature,
                    nature,
                    case.text.chars().take(90).collect::<String>()
                ));
            }
        }
        for (key, tally) in &by {
            println!("{key:<16} {}", tally.line());
        }
        // RECOGNIZE_DUMP: writes each case's guesses (id -> nature, line), for other benches.
        if let Ok(path) = std::env::var("RECOGNIZE_DUMP") {
            std::fs::write(path, serde_json::to_string(&guesses).unwrap()).unwrap();
        }
        if std::env::var("RECOGNIZE_MISSES").is_ok() {
            for m in misses {
                println!("{m}");
            }
        }
    }
}

// Texts of the website in this language (beta: translated from English, not reviewed yet).
// Same keys as fr.js; npm run check-i18n compares them.
export const DATES = { months: { Sep: 'set.', Oct: 'out.' }, since: 'desde {d} {m}', done: 'concluído em {d} {m}', day: '{d} {m}{t}' };

export const EL = {
    'Download for Windows': 'Baixar para Windows',
    'Watch one idea <em>grow</em>.': 'Veja uma ideia <em>crescer</em>.',
    'It starts <em>bare</em>.': 'Ela começa <em>nua</em>.',
    'Then it <em>grows</em>.': 'Depois <em>cresce</em>.',
    'Old leaves fall, and <em>stay</em>.': 'As folhas velhas caem, mas <em>ficam</em>.',
    'Then it <em>flowers</em> again.': 'E então <em>floresce</em> de novo.',
    'Your <em>turn</em>.': 'Sua <em>vez</em>.',
    'Press <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd> to add a case · <kbd>Enter</kbd> suggests next steps · <kbd>Esc</kbd> starts over':
      'Pressione <kbd>I</kbd> <kbd>T</kbd> <kbd>Q</kbd> <kbd>D</kbd> <kbd>R</kbd> para adicionar um caso · <kbd>Enter</kbd> sugere os próximos passos · <kbd>Esc</kbd> recomeça',
    'Work with your <em>agents</em>.': 'Trabalhe com seus <em>agentes</em>.',
    'Inside a <em>case</em>.': 'Dentro de um <em>caso</em>.',
    '<span class="mono">Home · example data</span>Your day at a glance: what you changed, what your agents wrote, what waits for you.':
      '<span class="mono">Início · dados de exemplo</span>Seu dia de relance: o que você mudou, o que seus agentes escreveram, o que está esperando por você.',
    '<span class="mono">Canvas · example data</span>A case and its sub-cases, laid out by hand.':
      '<span class="mono">Plano · dados de exemplo</span>Um caso e seus subcasos, dispostos à mão.',
    '<span class="mono">A result from an agent · example data</span>Claude Code hands in a comparison. You accept it, or ask for a rework.':
      '<span class="mono">Um resultado de um agente · dados de exemplo</span>O Claude Code entrega uma comparação. Você aceita ou pede um retrabalho.',
    'Your work stays <em>yours</em>.': 'Seu trabalho continua <em>seu</em>.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>. Installing with <code>winget install Humcase</code> avoids the warning.':
      'O instalador ainda não está assinado, então o Microsoft Defender SmartScreen mostra um aviso. Clique em <kbd>Mais informações</kbd> e depois em <kbd>Executar assim mesmo</kbd>. Instalar com <code>winget install Humcase</code> evita o aviso.',
    'The installer isn’t signed yet, so Microsoft Defender SmartScreen shows a warning. Click <kbd>More info</kbd>, then <kbd>Run anyway</kbd>.':
      'O instalador ainda não está assinado, então o Microsoft Defender SmartScreen mostra um aviso. Clique em <kbd>Mais informações</kbd> e depois em <kbd>Executar assim mesmo</kbd>.',
    'Every decision has <em>roots</em>.': 'Toda decisão tem suas <em>raízes</em>.',
    'Why <em>Humcase</em>?': 'Por que o <em>Humcase</em>?',
    'Plant your first <em>idea</em>.': 'Plante sua primeira <em>ideia</em>.',
    'Result ready, waiting for your decision<small>Codex cannot accept it for you</small>': 'Resultado pronto, esperando sua decisão<small>O Codex não pode aceitar por você</small>',
    '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · open source under AGPL-3.0': '<span class="brand__name" style="font-size:12px"><b>HUM</b>CASE</span> · código aberto sob AGPL-3.0',
  };

export const T = {
    'Contact': 'Contato', 'Legal notice': 'Aviso legal',
    'Terms': 'Termos',
    'Agents': 'Agentes', 'Questions': 'Perguntas', 'Question': 'Pergunta',
    // navigation, chapters, buttons
    'The cycle': 'O ciclo', 'The app': 'O app', 'Privacy': 'Privacidade', 'Download': 'Baixar',
    'Humcase on GitHub': 'Humcase no GitHub',
    'Bare': 'Nua', 'Grows': 'Cresce', 'Falls': 'Queda', 'Flowers': 'Floração',
    'Idea': 'Ideia', 'Task': 'Tarefa', 'Decision': 'Decisão', 'Research': 'Pesquisa',
    'Enter · Suggest': 'Enter · Sugerir', 'Esc · Start over': 'Esc · Recomeçar',
    'Replay': 'Repetir', 'Accept': 'Aceitar', 'Ask for a rework': 'Pedir retrabalho', 'Keep': 'Manter', 'Refuse': 'Recusar',
    'Get release news': 'Receber as novidades', 'Email for release news': 'E-mail para receber as novidades',
    'you@example.com': 'voce@exemplo.com.br', 'Language:': 'Idioma:', 'beta': 'beta',
    'PS C:\\Users\\you\\shop> ': 'PS C:\\Users\\voce\\loja> ',
    'Source on GitHub': 'Código no GitHub', 'Copied': 'Copiado', 'Select and copy': 'Selecione e copie',
    // hero and cycle
    'Track your ideas, tasks and decisions, and the work your agents hand in.':
      'Acompanhe suas ideias, tarefas e decisões, e o trabalho que seus agentes entregam.',
    'Working with agents leaves a trail of Markdown files, chats and folders. To find out why something was decided, you search, open and reread.':
      'Trabalhar com agentes deixa um rastro de arquivos Markdown, conversas e pastas. Para descobrir por que algo foi decidido, é preciso procurar, abrir e reler.',
    'In Humcase, every idea, decision, task and proof sits in one place you read at a glance, next to what led to it.':
      'No Humcase, cada ideia, decisão, tarefa e prova fica num só lugar, legível de relance, ao lado do que levou a ela.',
    'Free · Open source': 'Grátis · Código aberto',
    'Our example: redesigning a client’s online shop. Each leaf is a case: an idea, a task, a question or a decision.': 'Nosso exemplo: o redesenho da loja virtual de um cliente. Cada folha é um caso: uma ideia, uma tarefa, uma pergunta ou uma decisão.',
    'One line, typed between two meetings: redesign the online shop. That is already a case. No setup, no AI needed.':
      'Uma linha, digitada entre duas reuniões: redesenhar a loja virtual. Isso já é um caso. Sem configurar nada e sem precisar de IA.',
    'Each branch is a sub-case. With an AI service, Humcase suggests next steps from your words and creates only the ones you keep. Your agents add theirs, under their own name.':
      'Cada ramo é um subcaso. Com um serviço de IA, o Humcase sugere os próximos passos a partir das suas palavras e só cria os que você mantém. Seus agentes adicionam os deles, com o nome deles.',
    'Point at a framed leaf to read it': 'Aponte para uma folha com moldura para lê-la', 'Tap a framed leaf to read it': 'Toque numa folha com moldura para lê-la',
    'When a case is done, Humcase can suggest archiving it, and waits for your yes. An archived case stays readable: months later, you still see what was decided, and why.':
      'Quando um caso termina, o Humcase pode sugerir arquivá-lo e espera o seu sim. Um caso arquivado continua legível: meses depois, você ainda vê o que foi decidido e por quê.',
    'New ideas grow from what the old ones taught you, and each one remembers where it came from.':
      'As ideias novas nascem do que as antigas ensinaram, e cada uma lembra de onde veio.',
    // your turn
    'Tap a key to add a case.': 'Toque numa tecla para adicionar um caso.', 'An example: nothing is saved.': 'É só um exemplo: nada é salvo.',
    // agents
    'Claude Code, Codex and other agents connect to Humcase and write in the same cases as you, under their own name.':
      'O Claude Code, o Codex e outros agentes se conectam ao Humcase e escrevem nos mesmos casos que você, com o nome deles.',
    'Claude Code, asked to compare payment providers, writes a research case “Compare payment providers” into the Humcase case “Redesign the online shop” through Humcase’s MCP server. It then waits for your review.':
      'O Claude Code, a quem pediram para comparar provedores de pagamento, escreve um caso de pesquisa “Comparar provedores de pagamento” dentro do caso do Humcase “Redesenhar a loja virtual”, pelo servidor MCP do Humcase. Depois ele espera a sua revisão.',
    'An agent can’t accept its own work. It hands you a result: you accept it, or ask for a rework with a reason.':
      'Um agente não pode aceitar o próprio trabalho. Ele entrega um resultado: você aceita ou pede uma revisão com um motivo.',
    'Humcase doesn’t run your agents. It keeps what they hand in.': 'O Humcase não executa seus agentes. Ele guarda o que eles entregam.',
    'shop': 'loja', 'Resize the product photos': 'Redimensionar as fotos dos produtos', 'History': 'Histórico', 'You': 'Você',
    'created this task under “Redesign the online shop”': 'criou esta tarefa em “Redesenhar a loja virtual”', 'set it in progress': 'colocou em andamento',
    'added a result: 48 photos resized, originals kept': 'adicionou um resultado: 48 fotos redimensionadas, originais mantidos', 'Example data': 'Dados de exemplo',
    'accepted the result': 'aceitou o resultado', 'Done, accepted by you': 'Concluído, aceito por você', 'the decision is in the history': 'a decisão fica no histórico',
    'asked for a rework: “keep the white background”': 'pediu retrabalho: “manter o fundo branco”',
    'Back to Codex, in progress': 'De volta ao Codex, em andamento', 'your reason travels with the task': 'seu motivo acompanha a tarefa', 'just now': 'agora mesmo',
    'Humcase MCP server connected': 'servidor MCP do Humcase conectado',
    'Compare three payment providers for a small shop, and file it in Humcase': 'Compare três provedores de pagamento para uma loja pequena e registre no Humcase',
    'Reading the fees and docs of three providers…\n✓ 3 sources read, summary written': 'Lendo as taxas e a documentação de três provedores…\n✓ 3 fontes lidas, resumo escrito',
    '→ Humcase: research “Compare payment providers” added to “Redesign the online shop”': '→ Humcase: pesquisa “Comparar provedores de pagamento” adicionada a “Redesenhar a loja virtual”',
    '  It waits for your review: accept it, or ask for a rework.': '  Ela espera a sua revisão: aceite ou peça um retrabalho.',
    // the app, privacy, questions, final call
    'Write, dictate, sketch, add images. Arrange your cases on a canvas, or see them as cards, a list or a board. Work and personal stay apart.':
      'Escreva, dite, desenhe, adicione imagens. Organize seus casos num plano ou veja-os como cartões, lista ou quadro. Trabalho e vida pessoal ficam separados.',
    'On your computer': 'No seu computador', 'Your cases stay on your computer and work offline. Dictation and AI services with an API key need the internet.':
      'Seus casos ficam no seu computador e funcionam offline. O ditado e os serviços de IA com chave de API precisam de internet.',
    'Licensed under AGPL-3.0. Source code on request.': 'Licenciado sob a AGPL-3.0. Código-fonte sob solicitação.',
    'Licensed under AGPL-3.0.': 'Licenciado sob a AGPL-3.0.',
    'AI when you want it': 'IA só se você quiser',
    'Humcase works without AI. To add it: an API key or a local model.':
      'O Humcase funciona sem IA. Para adicioná-la: uma chave de API ou um modelo local.',
    'Is it free?': 'É grátis?',
    'Yes. Humcase is free and open source, under AGPL-3.0.':
      'Sim. O Humcase é grátis e de código aberto, sob a AGPL-3.0.',
    'Is it a note app or a task manager?': 'É um app de notas ou de tarefas?',
    'Both. A case can be an idea, a task, a question or a decision, and cases grow inside each other. Each one keeps where it came from.':
      'Os dois. Um caso pode ser uma ideia, uma tarefa, uma pergunta ou uma decisão, e os casos crescem uns dentro dos outros. Cada um guarda de onde veio.',
    'Which agents work with it?': 'Com quais agentes funciona?',
    'Claude Code, Codex, Cursor, opencode, and any agent that supports MCP. They write under their own name and can’t change your text unless you allow it.':
      'Claude Code, Codex, Cursor, opencode e qualquer agente compatível com MCP. Eles escrevem com o nome deles e não podem alterar o seu texto sem a sua permissão.',
    'Mac or Linux?': 'Mac ou Linux?', 'Windows 10 and 11.': 'Windows 10 e 11.',
    'Windows warns me when I install it. Why?': 'O Windows mostra um aviso quando instalo. Por quê?',
    'Only release news. Unsubscribe in one click.': 'Só novidades de versões. Cancele com um clique.',
    'Almost there: open the email we sent and confirm.': 'Quase lá: abra o e-mail que enviamos e confirme.',
    'This email address doesn’t look right.': 'Este endereço de e-mail não parece correto.',
    'That didn’t work. Try again in a moment.': 'Não deu certo. Tente de novo em instantes.',
    'The robot check couldn’t load. Turn off your blocker for humcase.com, then try again.': 'A verificação anti-robô não carregou. Desative seu bloqueador para humcase.com e tente de novo.',
    'Sending…': 'Enviando…',
    'The download isn’t open yet. Windows 10 and 11 · unsigned preview': 'O download ainda não está aberto. Windows 10 e 11 · prévia não assinada',
    'Humcase: every decision has roots': 'Humcase: toda decisão tem suas raízes', 'Humcase, home': 'Humcase, início', 'Site': 'Site', 'Footer': 'Rodapé', 'The cycle of an idea': 'O ciclo de uma ideia',
    'Chapters of the cycle': 'Capítulos do ciclo', 'Add a case to the example idea': 'Adicionar um caso à ideia de exemplo',
    'Example: Claude Code files research into Humcase': 'Exemplo: o Claude Code registra uma pesquisa no Humcase',
    'Example journal of a case': 'Exemplo de diário de um caso', 'Language': 'Idioma',
    // the legends of the 3D keyboard's keys
    'idea': 'ideia', 'task': 'tarefa', 'decision': 'decisão', 'research': 'pesquisa', 'sprout': 'germinar', 'empty': 'limpar',
    // the leaves' tags
    'archived': 'arquivado', 'Proposed by Humcase': 'Proposto pelo Humcase',
    'kept by you, proposed by Humcase': 'mantido por você, proposto pelo Humcase', 'Windows 10 and 11': 'Windows 10 e 11',
    'unsigned preview': 'prévia não assinada',
    'added by Claude': 'adicionado pelo Claude', 'added by Codex': 'adicionado pelo Codex', 'added by you': 'adicionado por você', 'open': 'aberto',
    'proposed by Humcase, kept by you': 'proposto pelo Humcase, mantido por você', 'result waiting for you': 'resultado esperando por você',
    'archived, not deleted': 'arquivado, não apagado', 'waiting for your review': 'esperando a sua revisão',
    'from your own words': 'a partir das suas palavras', 'not created yet': 'ainda não criado',
    // the example: a client's online shop
    'Shipping options in Europe': 'Opções de frete na Europa', 'grew from “Shipping options in Europe”': 'nasceu de “Opções de frete na Europa”',
    'Who buys on mobile?': 'Quem compra pelo celular?', 'Rewrite the product pages': 'Reescrever as páginas de produto',
    'Checkout on a single page': 'Checkout em uma única página', 'Export the old catalogue': 'Exportar o catálogo antigo',
    'Pick-up at the workshop': 'Retirada na oficina', 'Compare payment providers': 'Comparar provedores de pagamento',
    'Gift wrapping': 'Embalagem para presente', 'A loyalty card': 'Um cartão fidelidade', 'Free returns': 'Devolução grátis',
    'Write the shipping page': 'Escrever a página de frete', 'Fix the size guide': 'Corrigir o guia de tamanhos', 'Set up order emails': 'Configurar os e-mails de pedido',
    'Ship abroad?': 'Enviar para o exterior?', 'Keep the old links?': 'Manter os links antigos?', 'Which product goes first?': 'Qual produto vem primeiro?',
    'Free shipping over €50': 'Frete grátis acima de €50', 'Launch after the sales': 'Lançar depois da liquidação', 'Keep the same logo': 'Manter o mesmo logo',
    'Shops we admire': 'Lojas que admiramos', 'Delivery prices compared': 'Preços de entrega comparados', 'What customers ask most': 'O que os clientes mais perguntam',
    'Test checkout on an old phone': 'Testar o checkout num celular antigo', 'Card only, or PayPal too?': 'Só cartão ou PayPal também?',
    'Three shops we like, side by side': 'Três lojas de que gostamos, lado a lado', 'Send the preview to the client': 'Enviar a prévia ao cliente',
    'Which photo goes first?': 'Qual foto vem primeiro?', 'A 30-second product video': 'Um vídeo de produto de 30 segundos',
  };

export const ALT = {
    'Humcase home, work space: your day on a timeline of cases and agent writes, and a result from Claude Code waiting for your decision':
      'Início do Humcase, espaço de trabalho: seu dia numa linha do tempo de casos e escritas de agentes, e um resultado do Claude Code esperando sua decisão',
    'The canvas of the case “Redesign the online shop”: its sub-cases laid out by hand, each with its nature and status':
      'O plano do caso “Redesenhar a loja virtual”: seus subcasos dispostos à mão, cada um com sua natureza e seu status',
    'A research case written by Claude Code comparing payment providers: a comparison table and the buttons Accept and Ask for a rework':
      'Um caso de pesquisa escrito pelo Claude Code comparando provedores de pagamento: uma tabela comparativa e os botões Aceitar e Pedir retrabalho',
  };

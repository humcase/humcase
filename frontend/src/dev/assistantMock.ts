// Mockup of "Ask Humcase" (development only): scripted answers built from the real cases of the
// isolated profile, so sources open real cases and passages are quoted word for word. The real
// engine (retrieval on the computer, the AI chosen in Settings, checked citations) replaces it.
import i18n from '../i18n';
import type { AssistantAnswer, AssistantSource, AssistantTurn } from '../types/assistant';
import type { Note } from '../types';

const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const fold = (text: string) => text.toLowerCase().normalize('NFD').replace(/\p{M}/gu, '');
const STOP = new Set(['quoi', 'quel', 'quelle', 'avait', 'decide', 'pour', 'dans', 'avec', 'what', 'did', 'decided', 'about', 'which', 'est', 'qui', 'que', 'les', 'des', 'the', 'and', 'mes', 'my', 'pourquoi', 'why', 'pas', 'not']);

/** The sentence of a case holding `word`, word for word (the first sentence when none does). */
function passageOf(note: Note, words: string[]): string {
  const prose = (note.content ?? '').split('\n').filter(line => !line.trim().startsWith('|')).join('\n');
  const sentences = prose.replace(/[#*>-]/g, ' ').split(/(?<=[.!?])\s+|\n+/).map(s => s.replace(/\s+/g, ' ').trim()).filter(s => s.length > 12);
  const hit = sentences.find(s => words.some(w => fold(s).includes(w)));
  return (hit ?? sentences[0] ?? '').slice(0, 220);
}

function source(note: Note, words: string[]): AssistantSource {
  return { noteId: note.id, title: note.title, passage: passageOf(note, words) };
}

const byTitle = (notes: Note[], ...needles: string[]) =>
  notes.find(note => needles.some(needle => fold(note.title).includes(fold(needle))));

/** A scripted answer from the cases of the space; `noAi` plays the case of no AI connected. */
export async function mockAssistantAnswer(question: string, previous: AssistantTurn[], notes: Note[], noAi: boolean): Promise<AssistantAnswer> {
  await wait(900);
  const fr = i18n.language.startsWith('fr');
  const q = fold(question);
  const words = q.split(/[^\p{L}\p{N}]+/u).filter(w => w.length > 3 && !STOP.has(w));
  const lastAnswer = [...previous].reverse().find(turn => turn.answer?.kind === 'answer')?.answer;
  const service = 'Cerebras';

  if (noAi) {
    const close = notes.filter(note => words.some(w => fold(`${note.title} ${note.content ?? ''}`).includes(w))).slice(0, 4);
    return { kind: 'no_ai', parts: [], sources: close.map(note => source(note, words)) };
  }

  // Payment decision, and its follow-up "why not PayPal?".
  const research = byTitle(notes, 'solutions de paiement', 'payment providers');
  const decision = byTitle(notes, 'paiement sur une seule page', 'checkout on a single page');
  const shop = byTitle(notes, 'refonte de la boutique', 'redesign the online shop');
  if (/paypal|pourquoi|why/.test(q) && lastAnswer && research) {
    const sources = [source(research, ['paypal', 'mollie', 'stripe'])];
    return {
      kind: 'answer', sent: { service, excerpts: 1 }, sources,
      parts: fr
        ? [{ text: 'PayPal n’a pas été écarté : la comparaison met les trois solutions à égalité sur les cartes, Apple Pay et l’absence d’abonnement, et PayPal se met en place en une demi-journée.', cite: 1 }, { text: ' Le choix entre les trois reste à faire : la case attend votre décision.' }]
        : [{ text: 'PayPal was not ruled out: the comparison puts the three providers level on cards, Apple Pay and no monthly fee, and PayPal takes half a day to set up.', cite: 1 }, { text: ' The choice between them is still open: the case is waiting for your decision.' }],
    };
  }
  if (/paiement|payment|checkout|stripe/.test(q) && (research || decision)) {
    const sources = [decision, research].filter((n): n is Note => Boolean(n)).map(n => source(n, ['page', 'compte', 'account', 'stripe', 'paypal']));
    const parts = fr
      ? [{ text: 'Le paiement se fera sur une seule page : adresse, livraison et paiement ensemble, le compte devient facultatif après la commande.', cite: 1 }, { text: ' Pour le prestataire, Claude Code a comparé Stripe, PayPal et Mollie : les trois conviennent, sans abonnement.', cite: sources.length > 1 ? 2 : 1 }, { text: ' Ce résultat attend encore votre décision.' }]
      : [{ text: 'Checkout will be on a single page: address, delivery and payment together, an account becomes optional after the order.', cite: 1 }, { text: ' For the provider, Claude Code compared Stripe, PayPal and Mollie: all three fit, with no monthly fee.', cite: sources.length > 1 ? 2 : 1 }, { text: ' That result is still waiting for your decision.' }];
    return {
      kind: 'answer', parts, sources, sent: { service, excerpts: sources.length },
      action: shop ? { kind: 'create_sub_case', title: fr ? 'Choisir le prestataire de paiement' : 'Choose the payment provider', parentId: shop.id, parentTitle: shop.title } : undefined,
    };
  }

  // Results waiting for a decision.
  if (/decision|décision|attend|waiting|decide|valider|review/.test(q)) {
    const waiting = notes.filter(note => note.status === 'resolved').slice(0, 3);
    if (waiting.length) {
      const sources = waiting.map(note => source(note, []));
      return {
        kind: 'answer', sent: { service, excerpts: sources.length }, sources,
        parts: [{ text: fr ? `${waiting.length === 1 ? 'Un résultat attend' : `${waiting.length} résultats attendent`} votre décision : ` : `${waiting.length} result${waiting.length > 1 ? 's are' : ' is'} waiting for your decision: ` },
          ...waiting.map((note, index) => ({ text: `${index ? ', ' : ''}${note.title}`, cite: index + 1 })),
          { text: '.' }],
      };
    }
  }

  // Anything else: the cases sharing words with the question, or nothing.
  const hits = notes.filter(note => words.some(w => fold(`${note.title} ${note.content ?? ''}`).includes(w))).slice(0, 3);
  if (!hits.length) return { kind: 'not_found', parts: [], sources: [] };
  const sources = hits.map(note => source(note, words));
  return {
    kind: 'answer', sent: { service, excerpts: sources.length }, sources,
    parts: [{ text: fr ? `${hits.length} ${hits.length > 1 ? 'cases en parlent' : 'case en parle'} : ` : `${hits.length} case${hits.length > 1 ? 's mention' : ' mentions'} it: ` },
      ...hits.map((note, index) => ({ text: `${index ? ', ' : ''}${note.title}`, cite: index + 1 })), { text: '.' }],
  };
}

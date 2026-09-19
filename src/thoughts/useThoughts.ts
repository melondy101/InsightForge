import { useCallback, useEffect, useState } from 'react';

import { thoughtsApi, type ThoughtCard } from './api';
import { toCommandError, type CommandErrorShape } from '../native/command-client';

export interface ThoughtsModel {
  cards: ThoughtCard[];
  error: CommandErrorShape | null;
  actions: {
    upsert: (input: {
      id?: string;
      idea: string;
      sourceId: string;
      sourceName: string;
      relPath: string;
      headingPath: string;
      draftId: string;
      draftFilename: string;
    }) => Promise<void>;
    delete: (id: string) => Promise<void>;
    refresh: () => Promise<void>;
    dismissError: () => void;
  };
}

export function useThoughts(): ThoughtsModel {
  const [cards, setCards] = useState<ThoughtCard[]>([]);
  const [error, setError] = useState<CommandErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setCards(await thoughtsApi.list());
    } catch (err) {
      setError(toCommandError(err));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const upsert = useCallback(
    async (input: {
      id?: string;
      idea: string;
      sourceId: string;
      sourceName: string;
      relPath: string;
      headingPath: string;
      draftId: string;
      draftFilename: string;
    }) => {
      try {
        const card = await thoughtsApi.upsert(input);
        setCards((current) => {
          const index = current.findIndex((c) => c.id === card.id);
          if (index >= 0) {
            const next = [...current];
            next[index] = card;
            return next;
          }
          return [card, ...current];
        });
      } catch (err) {
        setError(toCommandError(err));
      }
    },
    [],
  );

  const deleteCard = useCallback(
    async (id: string) => {
      try {
        await thoughtsApi.delete(id);
        setCards((current) => current.filter((c) => c.id !== id));
      } catch (err) {
        setError(toCommandError(err));
      }
    },
    [],
  );

  const actions = {
    upsert,
    delete: deleteCard,
    refresh,
    dismissError: () => setError(null),
  };

  return { cards, error, actions };
}

import { useCallback, useEffect, useState } from 'react';

import { referencesApi, type Reference, type ReferencesErrorShape, toReferencesError } from './api';

export interface ReferencesModel {
  references: Reference[];
  error: ReferencesErrorShape | null;
  actions: {
    create: (draftId: string, fragmentId: number) => Promise<void>;
    adopt: (id: string) => Promise<void>;
    markSourceDeleted: (sourceId: string) => Promise<void>;
    dismissError: () => void;
  };
}

export function useReferences(draftId?: string): ReferencesModel {
  const [references, setReferences] = useState<Reference[]>([]);
  const [error, setError] = useState<ReferencesErrorShape | null>(null);

  const refresh = useCallback(async (id: string) => {
    try {
      setReferences(await referencesApi.list(id));
    } catch (err) {
      setError(toReferencesError(err));
    }
  }, []);

  useEffect(() => {
    if (draftId) {
      refresh(draftId);
    }
  }, [draftId, refresh]);

  const create = useCallback(
    async (createDraftId: string, fragmentId: number) => {
      try {
        const ref = await referencesApi.create(createDraftId, fragmentId);
        setReferences((current) => [ref, ...current]);
      } catch (err) {
        setError(toReferencesError(err));
      }
    },
    [],
  );

  const adopt = useCallback(
    async (id: string) => {
      try {
        await referencesApi.adopt(id);
        setReferences((current) =>
          current.map((r) => (r.id === id ? { ...r, adopted: true } : r)),
        );
      } catch (err) {
        setError(toReferencesError(err));
      }
    },
    [],
  );

  const markSourceDeleted = useCallback(
    async (sourceId: string) => {
      try {
        await referencesApi.markSourceDeleted(sourceId);
        setReferences((current) =>
          current.map((r) => (r.sourceId === sourceId ? { ...r, sourceDeleted: true } : r)),
        );
      } catch (err) {
        setError(toReferencesError(err));
      }
    },
    [],
  );

  const actions = {
    create,
    adopt,
    markSourceDeleted,
    dismissError: () => setError(null),
  };

  return { references, error, actions };
}

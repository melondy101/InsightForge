import { useCallback, useEffect, useState } from 'react';

import { egressApi, type AuditRecord, type EgressBundle } from './api';
import { toCommandError, type CommandErrorShape } from '../native/command-client';

export interface EgressModel {
  bundle: EgressBundle | null;
  records: AuditRecord[];
  error: CommandErrorShape | null;
  actions: {
    build: (draftContent: string, draftId: string, searchQuery?: { sourceId: string; query: string; limit: number }) => Promise<void>;
    log: (model: string, fragmentCount: number, fragmentIds: string[], status: string) => Promise<void>;
    refresh: () => Promise<void>;
    dismissError: () => void;
  };
}

export function useEgress(): EgressModel {
  const [bundle, setBundle] = useState<EgressBundle | null>(null);
  const [records, setRecords] = useState<AuditRecord[]>([]);
  const [error, setError] = useState<CommandErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRecords(await egressApi.list(50));
    } catch (err) {
      setError(toCommandError(err));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const build = useCallback(
    async (draftContent: string, id: string, searchQuery?: { sourceId: string; query: string; limit: number }) => {
      try {
        const result = await egressApi.build(draftContent, id, searchQuery);
        setBundle(result);
        // 自动记录审计。
        await egressApi.auditLog(
          'unknown',
          result.fragmentCount,
          result.fragments.map((f) => f.label),
          'built',
        );
        await refresh();
      } catch (err) {
        setError(toCommandError(err));
      }
    },
    [refresh],
  );

  const log = useCallback(
    async (model: string, fragmentCount: number, fragmentIds: string[], status: string) => {
      try {
        await egressApi.auditLog(model, fragmentCount, fragmentIds, status);
        await refresh();
      } catch (err) {
        setError(toCommandError(err));
      }
    },
    [refresh],
  );

  const actions = {
    build,
    log,
    refresh,
    dismissError: () => setError(null),
  };

  return { bundle, records, error, actions };
}

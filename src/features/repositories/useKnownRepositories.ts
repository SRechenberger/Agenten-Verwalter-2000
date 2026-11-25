import { useCallback, useEffect, useState } from 'react';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';
import { commandErrorText } from '@/lib/errors';
import { listRemoteRepositories } from '@/lib/remote';
import { addRepository, listRepositories, removeRepository } from '@/lib/repositories';

interface KnownRepositories {
  repositories: KnownRepository[];
  isLoading: boolean;
  /** Satz, warum die Liste nicht lädt; `null` ohne Fehler. */
  error: string | null;
  add: (path: string) => Promise<KnownRepository>;
  remove: (id: string) => Promise<void>;
}

/** Die Repositories dieses Rechners; mit `hostId` die des Hosts (nur lesen, `add` und `remove` ändern dort nichts). */
export function useKnownRepositories(hostId: string | null = null): KnownRepositories {
  const list = useCallback(
    (): Promise<KnownRepository[]> =>
      hostId === null ? listRepositories() : listRemoteRepositories(hostId),
    [hostId],
  );
  const [repositories, setRepositories] = useState<KnownRepository[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async (): Promise<void> => {
    try {
      setRepositories(await list());
      setError(null);
    } catch (reason: unknown) {
      console.error('Repositories konnten nicht geladen werden', reason);
      setRepositories([]);
      setError(`Repositories nicht ladbar: ${commandErrorText(reason)}`);
    }
  }, [list]);

  useEffect((): void => {
    list()
      .then((loaded: KnownRepository[]): void => {
        setRepositories(loaded);
        setError(null);
      })
      .catch((reason: unknown) => {
        console.error('Repositories konnten nicht geladen werden', reason);
        setError(`Repositories nicht ladbar: ${commandErrorText(reason)}`);
      })
      .finally((): void => {
        setIsLoading(false);
      });
  }, [list]);

  const add = useCallback(
    async (path: string): Promise<KnownRepository> => {
      const repository: KnownRepository = await addRepository(path);
      await reload();
      return repository;
    },
    [reload],
  );

  const remove = useCallback(
    async (id: string): Promise<void> => {
      await removeRepository(id);
      await reload();
    },
    [reload],
  );

  return { repositories, isLoading, error, add, remove };
}

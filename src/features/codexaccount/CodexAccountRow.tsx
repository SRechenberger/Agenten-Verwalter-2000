import type { ReactElement } from 'react';
import { AccountRowView } from '@/features/account/AccountRowView';
import { useCodexAccount } from '@/features/codexaccount/useCodexAccount';

export function CodexAccountRow(): ReactElement {
  const { status, login } = useCodexAccount();
  return (
    <AccountRowView
      settingId="codexAccount"
      errorPrefix="Codex-Konto"
      loginHint="Im Browser fortfahren."
      status={status}
      onLogin={login}
    />
  );
}

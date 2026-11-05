import type { ReactElement } from 'react';
import { AccountRowView } from '@/features/account/AccountRowView';
import { useAccount } from '@/features/account/useAccount';

export function AccountRow(): ReactElement {
  const { status, login } = useAccount();
  return (
    <AccountRowView
      settingId="claudeAccount"
      errorPrefix="Konto nicht lesbar"
      loginHint="Im Konsolenfenster und im Browser fortfahren."
      status={status}
      onLogin={login}
    />
  );
}

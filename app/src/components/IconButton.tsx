import type { ReactNode } from 'react';

interface IconButtonProps {
  children: ReactNode;
  title: string;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
}

export function IconButton({ children, title, onClick, disabled, active }: IconButtonProps) {
  return (
    <button
      className={active ? 'icon-button active' : 'icon-button'}
      title={title}
      aria-label={title}
      onClick={onClick}
      disabled={disabled}
      type="button"
    >
      {children}
    </button>
  );
}

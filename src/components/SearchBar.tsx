import { useEffect, useRef } from "react";

interface Props {
  value: string;
  onChange: (value: string) => void;
  isSearching: boolean;
  placeholder?: string;
}

export function SearchBar({ value, onChange, isSearching, placeholder }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="relative flex items-center border-b border-zinc-200 bg-white px-4">
      <svg
        className="mr-3 h-4 w-4 shrink-0 text-zinc-400"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M21 21l-4.35-4.35M17 11A6 6 0 1 1 5 11a6 6 0 0 1 12 0z"
        />
      </svg>
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? "Search highlights…"}
        className="flex-1 py-4 text-base text-zinc-900 placeholder-zinc-400 outline-none bg-transparent"
      />
      {isSearching && (
        <div className="h-3 w-3 animate-spin rounded-full border-2 border-zinc-300 border-t-zinc-600" />
      )}
    </div>
  );
}

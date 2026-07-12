"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Search } from "lucide-react";

import { Input } from "@/components/ui/input";

export function SortIndicator({
  active,
  order,
}: {
  active: boolean;
  order: "asc" | "desc";
}) {
  if (!active) return null;
  return order === "asc" ? (
    <ArrowUp className="ml-1 inline size-3" aria-hidden="true" />
  ) : (
    <ArrowDown className="ml-1 inline size-3" aria-hidden="true" />
  );
}

function FilterBar({
  search,
  onSearchChange,
  onSearchCommit,
  placeholder,
  tabs,
  activeTab,
  onTabChange,
}: {
  search: string;
  onSearchChange: (v: string) => void;
  onSearchCommit: () => void;
  placeholder: string;
  tabs: { key: string; label: string; count?: number }[];
  activeTab: string;
  onTabChange: (key: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
      <div className="relative w-full sm:max-w-xs">
        <Search
          className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
        <Input
          name="rewards-table-search"
          type="search"
          autoComplete="off"
          aria-label={placeholder}
          className="h-8 pl-8 text-sm"
          placeholder={placeholder}
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onSearchCommit();
          }}
          onBlur={onSearchCommit}
        />
      </div>
      <div className="flex flex-wrap gap-1">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            type="button"
            className={
              "rounded-md px-2.5 py-1 text-xs font-medium transition-colors " +
              (activeTab === tab.key
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-muted/80")
            }
            onClick={() => onTabChange(tab.key)}
            aria-pressed={activeTab === tab.key}
          >
            {tab.label}
            {typeof tab.count === "number" ? (
              <span className="ml-1 opacity-70">{tab.count}</span>
            ) : null}
          </button>
        ))}
      </div>
    </div>
  );
}

export function DebouncedFilterBar({
  initialSearch,
  onSearchChange,
  placeholder,
  tabs,
  activeTab,
  onTabChange,
}: {
  initialSearch: string;
  onSearchChange: (value: string) => void;
  placeholder: string;
  tabs: { key: string; label: string; count?: number }[];
  activeTab: string;
  onTabChange: (key: string) => void;
}) {
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const onSearchChangeRef = useRef(onSearchChange);
  const lastCommittedRef = useRef(initialSearch);
  const [search, setSearch] = useState(initialSearch);
  const [lastInitialSearch, setLastInitialSearch] = useState(initialSearch);

  // 外部搜索词变化时同步到内部状态（render 期调整，避免 effect setState 与 key remount 失焦）。
  if (initialSearch !== lastInitialSearch) {
    setLastInitialSearch(initialSearch);
    setSearch(initialSearch);
  }

  useEffect(() => {
    lastCommittedRef.current = initialSearch;
  }, [initialSearch]);

  useEffect(() => {
    onSearchChangeRef.current = onSearchChange;
  }, [onSearchChange]);

  useEffect(() => () => clearTimeout(debounceRef.current), []);

  const commitSearch = useCallback((value: string) => {
    if (value === lastCommittedRef.current) return;
    lastCommittedRef.current = value;
    onSearchChangeRef.current(value);
  }, []);

  const handleSearchChange = useCallback(
    (value: string) => {
      setSearch(value);
      clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => commitSearch(value), 400);
    },
    [commitSearch],
  );
  const handleSearchCommit = useCallback(() => {
    clearTimeout(debounceRef.current);
    commitSearch(search);
  }, [commitSearch, search]);

  return (
    <FilterBar
      search={search}
      onSearchChange={handleSearchChange}
      onSearchCommit={handleSearchCommit}
      placeholder={placeholder}
      tabs={tabs}
      activeTab={activeTab}
      onTabChange={onTabChange}
    />
  );
}
